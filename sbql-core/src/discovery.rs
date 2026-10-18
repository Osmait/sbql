//! Finding databases that are already running in Docker.
//!
//! The common case this exists for: you `cd` into a project, its
//! `docker compose` stack is already up, and connecting to its database means
//! re-typing a host, a port and a password that are all sitting right there in
//! the container. This asks Docker what is running and offers those databases
//! as ready-to-use connections.
//!
//! Running containers are the source of truth rather than the compose file.
//! A compose file states intent: it can be stale, use `${VAR}` interpolation
//! and an `.env` file, and — most importantly — a service with no published
//! `ports:` is not reachable from the host at all, so offering it produces a
//! connection that can only fail. A running container carries the *actual*
//! published port and the *actual* environment, and containers started by
//! compose keep labels naming their project and its directory, so "the
//! databases of the stack in this folder" is answerable without parsing YAML.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;
use uuid::Uuid;

use crate::config::ConnectionConfig;
use crate::pool::DbBackend;

/// How long to wait on the Docker CLI before giving up.
///
/// Discovery is a convenience that runs at startup, so it must never be what
/// makes the UI feel stuck — an unreachable daemon can otherwise hang for a
/// long time.
const DOCKER_TIMEOUT: Duration = Duration::from_secs(3);

/// Namespace for the v5 UUIDs derived from container ids.
///
/// Derived rather than random so a re-scan re-identifies the same container:
/// a fresh id every scan would orphan the open pool and the cached password of
/// a connection the user is currently using.
const DISCOVERY_NAMESPACE: Uuid = Uuid::from_u128(0x5b91_1a0e_7c42_4f9d_9a17_5d3b_6e8c_2f10);

/// Where a discovered database came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoverySource {
    /// A container started by `docker compose`.
    Compose {
        /// The compose project name — the stack the container belongs to.
        project: String,
        /// The service name within that stack, which is also what the
        /// connection ends up called.
        service: String,
        /// Whether the compose project is rooted at the directory sbql was
        /// opened in. Those are listed first: they are almost always the ones
        /// the user came for.
        here: bool,
    },
    /// A container started outside compose.
    Container {
        /// The container name, leading `/` stripped.
        name: String,
    },
}

impl DiscoverySource {
    /// Whether this is the stack of the directory sbql was opened in.
    pub fn is_here(&self) -> bool {
        matches!(self, Self::Compose { here: true, .. })
    }

    /// A short label for a client to show next to the connection.
    pub fn label(&self) -> String {
        match self {
            Self::Compose { project, here, .. } => {
                if *here {
                    "compose (here)".to_owned()
                } else {
                    format!("compose: {project}")
                }
            }
            Self::Container { .. } => "docker".to_owned(),
        }
    }
}

/// A database found running in Docker, as a client sees it.
///
/// Carries no password on purpose. The scraped password stays inside the
/// crate, in `Core`'s session cache, so it cannot ride along in a
/// [`CoreEvent`](crate::CoreEvent) — which the TUI worker debug-logs.
#[derive(Debug, Clone)]
pub struct DiscoveredConnection {
    /// Ready to hand to [`CoreCommand::Connect`](crate::CoreCommand::Connect)
    /// as-is — host, port and database are the container's real, published
    /// ones. The id is derived from the container id, so it survives a re-scan.
    pub config: ConnectionConfig,
    /// Which stack or container it came from, for labelling and ordering.
    pub source: DiscoverySource,
}

/// A discovered connection together with the password scraped from the
/// container's environment.
///
/// Kept as a separate type so a password can never ride along inside a
/// `CoreEvent`: the TUI worker debug-logs every event it forwards, which would
/// write real database passwords into the log file. The core keeps the
/// password in its in-memory cache and hands clients only the
/// [`DiscoveredConnection`].
#[derive(Clone)]
pub(crate) struct DiscoveredCredentials {
    pub(crate) connection: DiscoveredConnection,
    pub(crate) password: String,
}

/// The point of this type is that the scraped password does not travel; a
/// derived `Debug` would undo that the first time one of these was logged.
impl std::fmt::Debug for DiscoveredCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiscoveredCredentials")
            .field("connection", &self.connection)
            .field(
                "password",
                &crate::connection_spec::Redacted(!self.password.is_empty()),
            )
            .finish()
    }
}

/// Ask Docker what databases are running, most relevant first.
///
/// `dir` is the directory sbql was opened in; containers belonging to the
/// compose project rooted there are listed first. Returns an empty list — never
/// an error — when Docker is missing, unreachable, or has nothing to offer:
/// this is a convenience, and a red banner about a daemon the user never asked
/// about would be noise.
pub(crate) async fn discover(dir: &Path) -> Vec<DiscoveredCredentials> {
    let ids = match docker(&["ps", "--quiet", "--no-trunc"]).await {
        Some(out) if !out.trim().is_empty() => out,
        Some(_) => return Vec::new(),
        None => return Vec::new(),
    };

    let mut args = vec!["inspect"];
    args.extend(ids.split_whitespace());
    let Some(json) = docker(&args).await else {
        return Vec::new();
    };

    parse_inspect(&json, dir)
}

/// Run the Docker CLI, returning its stdout, or `None` if it could not be run.
async fn docker(args: &[&str]) -> Option<String> {
    let run = tokio::process::Command::new("docker")
        .args(args)
        .kill_on_drop(true)
        .output();

    match tokio::time::timeout(DOCKER_TIMEOUT, run).await {
        Ok(Ok(out)) if out.status.success() => {
            Some(String::from_utf8_lossy(&out.stdout).into_owned())
        }
        Ok(Ok(out)) => {
            tracing::debug!(
                "docker {:?} exited with {}: {}",
                args.first(),
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            );
            None
        }
        Ok(Err(e)) => {
            tracing::debug!("docker {:?} could not be run: {e}", args.first());
            None
        }
        Err(_) => {
            tracing::warn!(
                "docker {:?} timed out after {DOCKER_TIMEOUT:?}",
                args.first()
            );
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Parsing `docker inspect`
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct Container {
    #[serde(rename = "Id", default)]
    id: String,
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "Config", default)]
    config: ContainerConfig,
    #[serde(rename = "NetworkSettings", default)]
    network_settings: NetworkSettings,
}

#[derive(Deserialize, Default)]
struct ContainerConfig {
    #[serde(rename = "Image", default)]
    image: String,
    #[serde(rename = "Env", default)]
    env: Option<Vec<String>>,
    #[serde(rename = "Labels", default)]
    labels: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Default)]
struct NetworkSettings {
    /// A container port maps to `null` when nothing is published for it, which
    /// is exactly the case that must not become an offered connection.
    #[serde(rename = "Ports", default)]
    ports: Option<HashMap<String, Option<Vec<PortBinding>>>>,
}

#[derive(Deserialize)]
struct PortBinding {
    #[serde(rename = "HostPort", default)]
    host_port: Option<String>,
}

/// Turn `docker inspect` output into connections, ordered with the current
/// directory's compose project first.
///
/// Split from [`discover`] so the mapping is testable without Docker.
pub(crate) fn parse_inspect(json: &str, dir: &Path) -> Vec<DiscoveredCredentials> {
    let containers: Vec<Container> = match serde_json::from_str(json) {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("could not read `docker inspect` output: {e}");
            return Vec::new();
        }
    };

    let mut found: Vec<DiscoveredCredentials> = containers
        .iter()
        .filter_map(|c| to_connection(c, dir))
        .collect();

    // The stack in this directory first, then other compose projects, then
    // loose containers; ties keep a stable name order so the list does not
    // reshuffle between scans.
    found.sort_by(|a, b| {
        let rank = |s: &DiscoverySource| match s {
            // `is_here()` rather than repeating `here: true` — the predicate
            // that decides "the stack the user came for" should have exactly
            // one definition, and this was the only production caller it was
            // missing.
            DiscoverySource::Compose { .. } if s.is_here() => 0,
            DiscoverySource::Compose { .. } => 1,
            DiscoverySource::Container { .. } => 2,
        };
        rank(&a.connection.source)
            .cmp(&rank(&b.connection.source))
            .then_with(|| a.connection.config.name.cmp(&b.connection.config.name))
    });
    found
}

fn to_connection(container: &Container, dir: &Path) -> Option<DiscoveredCredentials> {
    let (backend, container_port) = backend_for_image(&container.config.image)?;
    // No published port means the database is only reachable from inside the
    // Docker network. It is running, but not by us.
    let host_port = published_port(&container.network_settings, container_port)?;

    let env = env_map(container.config.env.as_deref().unwrap_or(&[]));
    let labels = container.config.labels.clone().unwrap_or_default();

    let service = labels.get("com.docker.compose.service").cloned();
    let project = labels.get("com.docker.compose.project").cloned();
    let container_name = container.name.trim_start_matches('/').to_owned();

    let source = match (project, service) {
        (Some(project), Some(service)) => {
            let here = labels
                .get("com.docker.compose.project.working_dir")
                .is_some_and(|wd| Path::new(wd) == dir);
            DiscoverySource::Compose {
                project,
                service,
                here,
            }
        }
        _ => DiscoverySource::Container {
            name: container_name,
        },
    };

    let name = match &source {
        DiscoverySource::Compose { service, .. } => service.clone(),
        DiscoverySource::Container { name } => name.clone(),
    };

    let creds = credentials(backend, &env);
    let mut config = build_config(backend, &name, host_port, &creds);
    // Derived from the container id so a re-scan keeps the same identity.
    config.id = Uuid::new_v5(&DISCOVERY_NAMESPACE, container.id.as_bytes());

    Some(DiscoveredCredentials {
        connection: DiscoveredConnection { config, source },
        password: creds.password,
    })
}

/// `["KEY=value", ...]` as Docker reports it.
fn env_map(env: &[String]) -> HashMap<&str, &str> {
    env.iter()
        .filter_map(|entry| entry.split_once('='))
        .collect()
}

/// The host port published for `container_port`, if any.
fn published_port(net: &NetworkSettings, container_port: u16) -> Option<u16> {
    net.ports
        .as_ref()?
        .get(&format!("{container_port}/tcp"))?
        .as_ref()?
        .iter()
        .find_map(|b| b.host_port.as_deref()?.parse().ok())
}

/// Which backend an image runs, and the port it listens on inside the
/// container.
///
/// Matched on the repository path with the registry host and tag stripped, so
/// `mcr.microsoft.com/mssql/server:2022-latest` and a bare `postgres` are both
/// recognised. Popular drop-in forks and flavours count: they speak the same
/// wire protocol, which is all sbql needs.
fn backend_for_image(image: &str) -> Option<(DbBackend, u16)> {
    let repo = image
        .rsplit_once(':')
        // A tag cannot contain '/', so a slash after the colon means that
        // colon was a registry port (`localhost:5000/postgres`), not a tag.
        .filter(|(_, tag)| !tag.contains('/'))
        .map_or(image, |(repo, _)| repo)
        .to_ascii_lowercase();

    // Longest-lived rule first: match on the last path segment where possible,
    // and fall back to a substring for vendor-prefixed images.
    let last = repo.rsplit('/').next().unwrap_or(&repo);

    let backend = match last {
        "postgres" | "postgresql" | "pgvector" => (DbBackend::Postgres, 5432),
        "mysql" | "mariadb" | "percona" => (DbBackend::Mysql, 3306),
        "mongo" | "mongodb" => (DbBackend::MongoDb, 27017),
        "redis" | "valkey" | "keydb" => (DbBackend::Redis, 6379),
        _ if repo.contains("postgis") || repo.contains("timescaledb") => {
            (DbBackend::Postgres, 5432)
        }
        _ if repo.contains("mysql") || repo.contains("mariadb") => (DbBackend::Mysql, 3306),
        _ if repo.contains("mongodb") => (DbBackend::MongoDb, 27017),
        _ if repo.contains("redis") || repo.contains("valkey") => (DbBackend::Redis, 6379),
        _ if repo.contains("mssql/server") || repo.contains("azure-sql-edge") => {
            (DbBackend::SqlServer, 1433)
        }
        _ if repo.contains("dynamodb-local") => (DbBackend::DynamoDb, 8000),
        _ => return None,
    };
    Some(backend)
}

/// What a container's environment says its credentials are.
struct Credentials {
    user: String,
    password: String,
    database: String,
}

/// Read the credentials out of a container's environment.
///
/// The variable names are the ones each official image documents for
/// initialising itself, so this reads exactly what the database was created
/// with. Where an image supports a randomised or absent password there is
/// nothing to read, and the user is left to type it.
fn credentials(backend: DbBackend, env: &HashMap<&str, &str>) -> Credentials {
    let get = |key: &str| env.get(key).map(|v| (*v).to_owned());
    let any = |keys: &[&str]| keys.iter().find_map(|k| get(k));

    match backend {
        DbBackend::Postgres => {
            let user = get("POSTGRES_USER").unwrap_or_else(|| "postgres".to_owned());
            Credentials {
                database: get("POSTGRES_DB").unwrap_or_else(|| user.clone()),
                password: get("POSTGRES_PASSWORD").unwrap_or_default(),
                user,
            }
        }
        DbBackend::Mysql => {
            // A non-root user only exists when both halves were given; MariaDB
            // accepts the same variables under its own prefix.
            let user = any(&["MYSQL_USER", "MARIADB_USER"]);
            let user_pw = any(&["MYSQL_PASSWORD", "MARIADB_PASSWORD"]);
            match (user, user_pw) {
                (Some(user), Some(password)) => Credentials {
                    user,
                    password,
                    database: any(&["MYSQL_DATABASE", "MARIADB_DATABASE"]).unwrap_or_default(),
                },
                _ => Credentials {
                    user: "root".to_owned(),
                    password: any(&["MYSQL_ROOT_PASSWORD", "MARIADB_ROOT_PASSWORD"])
                        .unwrap_or_default(),
                    database: any(&["MYSQL_DATABASE", "MARIADB_DATABASE"]).unwrap_or_default(),
                },
            }
        }
        DbBackend::MongoDb => Credentials {
            user: get("MONGO_INITDB_ROOT_USERNAME").unwrap_or_default(),
            password: get("MONGO_INITDB_ROOT_PASSWORD").unwrap_or_default(),
            database: get("MONGO_INITDB_DATABASE").unwrap_or_else(|| "admin".to_owned()),
        },
        DbBackend::Redis => Credentials {
            user: String::new(),
            password: any(&["REDIS_PASSWORD", "VALKEY_PASSWORD"]).unwrap_or_default(),
            database: "0".to_owned(),
        },
        DbBackend::SqlServer => Credentials {
            user: "sa".to_owned(),
            password: any(&["MSSQL_SA_PASSWORD", "SA_PASSWORD"]).unwrap_or_default(),
            database: "master".to_owned(),
        },
        // DynamoDB Local ignores credentials but the SDK insists on some, and
        // `database` is where this backend keeps its region.
        DbBackend::DynamoDb => Credentials {
            user: "local".to_owned(),
            password: "local".to_owned(),
            database: get("AWS_REGION").unwrap_or_else(|| "us-east-1".to_owned()),
        },
        // Not a networked server, so it can never come from a container.
        DbBackend::Sqlite => Credentials {
            user: String::new(),
            password: String::new(),
            database: String::new(),
        },
    }
}

fn build_config(
    backend: DbBackend,
    name: &str,
    port: u16,
    creds: &Credentials,
) -> ConnectionConfig {
    // Always the loopback address: a published port is reachable there
    // regardless of which interface Docker bound it to.
    const HOST: &str = "127.0.0.1";
    match backend {
        DbBackend::Postgres => {
            ConnectionConfig::new_postgres(name, HOST, port, &creds.user, &creds.database)
        }
        DbBackend::Mysql => {
            ConnectionConfig::new_mysql(name, HOST, port, &creds.user, &creds.database)
        }
        DbBackend::MongoDb => {
            let mut c = ConnectionConfig::new_mongodb(name, HOST, port, &creds.database);
            c.user.clone_from(&creds.user);
            c
        }
        DbBackend::Redis => {
            let mut c = ConnectionConfig::new_redis(name, HOST, port);
            c.database.clone_from(&creds.database);
            c
        }
        DbBackend::SqlServer => {
            ConnectionConfig::new_sqlserver(name, HOST, port, &creds.user, &creds.database)
        }
        DbBackend::DynamoDb => {
            let mut c = ConnectionConfig::new_dynamodb(name, HOST, port, &creds.database);
            c.user.clone_from(&creds.user);
            c
        }
        DbBackend::Sqlite => ConnectionConfig::new_sqlite(name, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Modelled on real `docker inspect` output.
    ///
    /// The passwords spell out that they are fixtures: a value that merely
    /// looks invented still trips a secret scanner, and a scanner that cries
    /// wolf over test data is one people learn to ignore.
    fn inspect_json() -> String {
        r#"[
          {
            "Id": "aaa111",
            "Name": "/shop-postgres-1",
            "Config": {
              "Image": "postgres:16-alpine",
              "Env": ["POSTGRES_PASSWORD=fixture-not-a-real-password", "POSTGRES_DB=shop", "POSTGRES_USER=admin", "PATH=/usr/bin"],
              "Labels": {
                "com.docker.compose.project": "shop",
                "com.docker.compose.service": "postgres",
                "com.docker.compose.project.working_dir": "/home/dev/shop"
              }
            },
            "NetworkSettings": { "Ports": { "5432/tcp": [{"HostIp": "0.0.0.0", "HostPort": "5433"}] } }
          },
          {
            "Id": "bbb222",
            "Name": "/other-db-1",
            "Config": {
              "Image": "mariadb:10.11",
              "Env": ["MYSQL_ROOT_PASSWORD=fixture-not-a-real-password", "MYSQL_DATABASE=misp"],
              "Labels": {
                "com.docker.compose.project": "other",
                "com.docker.compose.service": "db",
                "com.docker.compose.project.working_dir": "/home/dev/other"
              }
            },
            "NetworkSettings": { "Ports": { "3306/tcp": null } }
          },
          {
            "Id": "ccc333",
            "Name": "/loose-redis",
            "Config": {
              "Image": "valkey/valkey:7.2",
              "Env": ["REDIS_PASSWORD=fixture-not-a-real-cache-password"],
              "Labels": {}
            },
            "NetworkSettings": { "Ports": { "6379/tcp": [{"HostIp": "127.0.0.1", "HostPort": "6380"}] } }
          },
          {
            "Id": "ddd444",
            "Name": "/webserver",
            "Config": { "Image": "nginx:latest", "Env": [], "Labels": {} },
            "NetworkSettings": { "Ports": { "80/tcp": [{"HostIp": "0.0.0.0", "HostPort": "8080"}] } }
          }
        ]"#
        .to_owned()
    }

    #[test]
    fn reads_a_compose_postgres_with_its_published_port() {
        let found = parse_inspect(&inspect_json(), Path::new("/home/dev/shop"));

        let pg = &found[0];
        assert_eq!(pg.connection.config.backend, DbBackend::Postgres);
        assert_eq!(pg.connection.config.name, "postgres");
        assert_eq!(pg.connection.config.host, "127.0.0.1");
        // The published host port, not the port inside the container.
        assert_eq!(pg.connection.config.port, 5433);
        assert_eq!(pg.connection.config.user, "admin");
        assert_eq!(pg.connection.config.database, "shop");
        assert_eq!(pg.password, "fixture-not-a-real-password");
        assert!(pg.connection.source.is_here());
    }

    /// A running container with no published port is not reachable from the
    /// host, so offering it would only ever produce a failed connection.
    #[test]
    fn skips_a_database_with_no_published_port() {
        let found = parse_inspect(&inspect_json(), Path::new("/home/dev/shop"));

        assert!(
            !found
                .iter()
                .any(|d| d.connection.config.backend == DbBackend::Mysql),
            "the unpublished mariadb must not be offered"
        );
    }

    #[test]
    fn ignores_containers_that_are_not_databases() {
        let found = parse_inspect(&inspect_json(), Path::new("/home/dev/shop"));
        assert!(!found
            .iter()
            .any(|d| d.connection.config.name == "webserver"));
    }

    /// The stack of the directory sbql was opened in comes first.
    #[test]
    fn orders_the_current_directorys_stack_first() {
        let found = parse_inspect(&inspect_json(), Path::new("/home/dev/shop"));

        assert_eq!(found.len(), 2);
        assert!(found[0].connection.source.is_here());
        assert!(!found[1].connection.source.is_here());
        assert!(matches!(
            found[1].connection.source,
            DiscoverySource::Container { .. }
        ));
    }

    /// Run from somewhere else, the same containers are still offered — just
    /// without the "here" precedence.
    #[test]
    fn a_different_directory_marks_nothing_as_here() {
        let found = parse_inspect(&inspect_json(), Path::new("/somewhere/else"));
        assert!(found.iter().all(|d| !d.connection.source.is_here()));
    }

    #[test]
    fn a_loose_container_is_named_after_the_container() {
        let found = parse_inspect(&inspect_json(), Path::new("/home/dev/shop"));
        let redis = found
            .iter()
            .find(|d| d.connection.config.backend == DbBackend::Redis)
            .expect("the published valkey");

        assert_eq!(redis.connection.config.name, "loose-redis");
        assert_eq!(redis.connection.config.port, 6380);
        assert_eq!(redis.password, "fixture-not-a-real-cache-password");
        assert_eq!(
            redis.connection.source,
            DiscoverySource::Container {
                name: "loose-redis".into()
            }
        );
    }

    /// Prove the credentials are usable, not merely plausible.
    ///
    /// Everything else here checks the *shape* of what discovery produces
    /// against a fixture; only opening a real connection shows that the user
    /// and database it read out of the container are the ones the server will
    /// accept. Read-only: it opens a pool and pings.
    ///
    /// Ignored by default — it needs a running Docker with a database whose
    /// port is published, and it talks to whatever that database is:
    ///
    /// ```text
    /// cargo test -p sbql-core --lib discovery -- --ignored --nocapture
    /// ```
    // Run under `--nocapture`, where the printed report *is* what this test
    // produces: it names which discovered database answered. Nothing here runs
    // while a terminal UI holds the screen, which is what `print_stdout` exists
    // to protect.
    #[allow(clippy::print_stdout)]
    #[tokio::test]
    #[ignore = "needs a running Docker daemon with a published database"]
    async fn discovered_credentials_actually_connect() {
        #[allow(clippy::expect_used)]
        let cwd = std::env::current_dir().expect("a current directory");
        let found = discover(&cwd).await;
        if found.is_empty() {
            println!("no published databases running — nothing to prove");
            return;
        }

        let manager = crate::connection::ConnectionManager::default();
        for d in &found {
            let name = &d.connection.config.name;
            match manager
                .connect_with_password(&d.connection.config, &d.password)
                .await
            {
                Ok(()) => match manager.ping(d.connection.config.id).await {
                    Ok(()) => println!("  {name}: connected and answered a ping"),
                    Err(e) => panic!("{name}: connected but would not answer: {e}"),
                },
                Err(e) => panic!("{name}: discovered credentials were rejected: {e}"),
            }
            manager.disconnect(d.connection.config.id).await;
        }
    }

    /// The same container keeps the same id across scans, so an open pool and
    /// its cached password survive a refresh.
    #[test]
    fn ids_are_derived_from_the_container() {
        let first = parse_inspect(&inspect_json(), Path::new("/home/dev/shop"));
        let again = parse_inspect(&inspect_json(), Path::new("/home/dev/shop"));
        assert_eq!(first[0].connection.config.id, again[0].connection.config.id);
        assert_ne!(first[0].connection.config.id, first[1].connection.config.id);
    }

    #[test]
    fn recognises_the_images_people_actually_run() {
        for (image, expected) in [
            ("postgres:16-alpine", DbBackend::Postgres),
            ("postgres", DbBackend::Postgres),
            ("postgis/postgis:16-3.4", DbBackend::Postgres),
            ("timescale/timescaledb:latest-pg16", DbBackend::Postgres),
            ("localhost:5000/postgres", DbBackend::Postgres),
            ("mysql:8", DbBackend::Mysql),
            ("mariadb:10.11", DbBackend::Mysql),
            ("bitnami/mysql:8.0", DbBackend::Mysql),
            ("mongo:7", DbBackend::MongoDb),
            ("redis:7-alpine", DbBackend::Redis),
            ("valkey/valkey:7.2", DbBackend::Redis),
            (
                "mcr.microsoft.com/mssql/server:2022-latest",
                DbBackend::SqlServer,
            ),
            ("amazon/dynamodb-local:latest", DbBackend::DynamoDb),
        ] {
            assert_eq!(
                backend_for_image(image).map(|(b, _)| b),
                Some(expected),
                "{image}"
            );
        }

        for image in ["nginx:latest", "ghcr.io/misp/misp-docker/misp-core:latest"] {
            assert!(backend_for_image(image).is_none(), "{image}");
        }
    }

    #[test]
    fn mysql_prefers_the_named_user_over_root() {
        let env = HashMap::from([
            ("MYSQL_USER", "app"),
            ("MYSQL_PASSWORD", "apppw"),
            ("MYSQL_ROOT_PASSWORD", "rootpw"),
            ("MYSQL_DATABASE", "shop"),
        ]);
        let creds = credentials(DbBackend::Mysql, &env);
        assert_eq!(creds.user, "app");
        assert_eq!(creds.password, "apppw");
        assert_eq!(creds.database, "shop");
    }

    /// Half a user is no user: the image only creates one when both variables
    /// are present, so falling back to root is what actually works.
    #[test]
    fn mysql_falls_back_to_root_without_a_full_user() {
        let env = HashMap::from([("MYSQL_USER", "app"), ("MYSQL_ROOT_PASSWORD", "rootpw")]);
        let creds = credentials(DbBackend::Mysql, &env);
        assert_eq!(creds.user, "root");
        assert_eq!(creds.password, "rootpw");
    }

    #[test]
    fn postgres_defaults_match_the_images_own() {
        let creds = credentials(DbBackend::Postgres, &HashMap::new());
        assert_eq!(creds.user, "postgres");
        // The image defaults the database to the user name.
        assert_eq!(creds.database, "postgres");
        assert!(creds.password.is_empty());
    }

    #[test]
    fn a_registry_port_is_not_mistaken_for_a_tag() {
        assert_eq!(
            backend_for_image("registry.local:5000/mongo").map(|(b, _)| b),
            Some(DbBackend::MongoDb)
        );
    }

    /// Point discovery at the real Docker daemon and print what it found.
    ///
    /// Ignored by default because it needs a running Docker and its result
    /// depends on whatever happens to be up. Run it by hand when changing the
    /// image or credential tables:
    ///
    /// ```text
    /// cargo test -p sbql-core --lib discovery -- --ignored --nocapture
    /// ```
    ///
    /// Deliberately prints no passwords: this is a debugging aid, not a way to
    /// dump the machine's credentials into a terminal or a CI log.
    // Run under `--nocapture`, where the printed listing *is* the test's whole
    // product — there is nothing to assert about "whatever happens to be up".
    // Nothing here runs while a terminal UI holds the screen, which is what
    // `print_stdout` exists to protect.
    #[allow(clippy::print_stdout)]
    #[tokio::test]
    #[ignore = "needs a running Docker daemon"]
    async fn against_the_real_docker() {
        #[allow(clippy::expect_used)]
        let cwd = std::env::current_dir().expect("a current directory");
        let found = discover(&cwd).await;

        println!(
            "discovered {} database(s) from {}",
            found.len(),
            cwd.display()
        );
        for d in &found {
            let c = &d.connection.config;
            println!(
                "  {:?} {} → {}:{} db={:?} user={:?} [{}] password={}",
                c.backend,
                c.name,
                c.host,
                c.port,
                c.database,
                c.user,
                d.connection.source.label(),
                if d.password.is_empty() {
                    "none found"
                } else {
                    "found (not shown)"
                }
            );
        }
    }
}
