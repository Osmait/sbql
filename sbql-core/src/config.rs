use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Result, SbqlError};
use crate::pool::DbBackend;

/// Environment variable that relocates the connection file away from the
/// default `~/.config/sbql` (`~/Library/Application Support/sbql` on macOS).
pub const CONFIG_DIR_ENV: &str = "SBQL_CONFIG_DIR";

/// Set this to `1` (or `true`) to skip the OS credential store entirely.
/// Passwords then live in memory for the session and are never written anywhere.
pub const NO_KEYRING_ENV: &str = "SBQL_NO_KEYRING";

/// Fault injection for tests in other modules of this crate.
#[cfg(test)]
pub(crate) use store::fault as store_fault;

/// Whether passwords are persisted at all.
///
/// False when the crate was built without the `keyring` feature, or when
/// [`NO_KEYRING_ENV`] is set. Callers use this to phrase the UI honestly rather
/// than reporting an unusable store as a failure.
pub fn keyring_enabled() -> bool {
    cfg!(feature = "keyring") && !opted_out_of_keyring()
}

fn opted_out_of_keyring() -> bool {
    matches!(
        std::env::var(NO_KEYRING_ENV).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes")
    )
}

/// The credential store itself, isolated so the rest of the file never has to
/// care whether the `keyring` feature is compiled in.
mod store {
    use super::{opted_out_of_keyring, Result, SbqlError};

    pub(super) const SERVICE: &str = "sbql";
    pub(super) const SSH_SERVICE: &str = "sbql-ssh";

    /// Pretend the credential store is broken, for tests that need to exercise
    /// the failure path without a real one.
    ///
    /// Thread-local rather than global: the test suite runs in parallel, and a
    /// process-wide flag would leak into whatever else is running.
    #[cfg(test)]
    pub(crate) mod fault {
        use std::cell::Cell;

        thread_local! {
            static FORCED: Cell<bool> = const { Cell::new(false) };
        }

        /// Make every store operation on this thread fail until dropped.
        pub(crate) struct ForcedFailure;

        impl ForcedFailure {
            pub(crate) fn new() -> Self {
                FORCED.with(|f| f.set(true));
                Self
            }
        }

        impl Drop for ForcedFailure {
            fn drop(&mut self) {
                FORCED.with(|f| f.set(false));
            }
        }

        pub(super) fn active() -> bool {
            FORCED.with(|f| f.get())
        }
    }

    #[cfg(test)]
    fn forced_failure() -> Option<SbqlError> {
        fault::active()
            .then(|| SbqlError::Keyring("credential store unavailable (test)".to_string()))
    }

    #[cfg(not(test))]
    fn forced_failure() -> Option<SbqlError> {
        None
    }

    #[cfg(feature = "keyring")]
    mod backend {
        use super::*;
        use keyring::Entry;

        /// What to tell the user when the OS credential store cannot be reached.
        ///
        /// `keyring` surfaces this as a low-level platform error — on Linux a raw
        /// D-Bus message like "The name is not activatable" — which says nothing
        /// about what to do next. This lands in a single-line status bar, so it
        /// has to stay short; the underlying cause goes to the log instead.
        #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
        const NO_STORE_HINT: &str =
            "Secret Service unavailable or locked — see 'Linux credential storage' in the README";

        #[cfg(target_os = "macos")]
        const NO_STORE_HINT: &str = "the macOS Keychain could not be reached";

        #[cfg(not(any(
            target_os = "linux",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "macos"
        )))]
        const NO_STORE_HINT: &str = "no keyring backend is available on this platform";

        fn map_err(e: keyring::Error) -> SbqlError {
            match e {
                keyring::Error::PlatformFailure(ref cause)
                | keyring::Error::NoStorageAccess(ref cause) => {
                    tracing::warn!("credential store unreachable: {cause}");
                    SbqlError::Keyring(NO_STORE_HINT.to_string())
                }
                other => SbqlError::Keyring(other.to_string()),
            }
        }

        pub(super) fn set(service: &str, user: &str, password: &str) -> Result<()> {
            let entry = Entry::new(service, user).map_err(map_err)?;
            entry.set_password(password).map_err(map_err)
        }

        pub(super) fn get(service: &str, user: &str, label: &str) -> Result<String> {
            let entry = Entry::new(service, user).map_err(map_err)?;
            entry.get_password().map_err(|e| match e {
                keyring::Error::NoEntry => SbqlError::PasswordNotFound(label.to_string()),
                other => map_err(other),
            })
        }

        pub(super) fn delete(service: &str, user: &str) -> Result<()> {
            let entry = Entry::new(service, user).map_err(map_err)?;
            entry.delete_credential().map_err(map_err)
        }
    }

    /// Built without the `keyring` feature: nothing is stored, and nothing is
    /// reported as broken — the absence is the configured behaviour.
    #[cfg(not(feature = "keyring"))]
    mod backend {
        use super::*;

        pub(super) fn set(_service: &str, _user: &str, _password: &str) -> Result<()> {
            Ok(())
        }

        pub(super) fn get(_service: &str, _user: &str, label: &str) -> Result<String> {
            Err(SbqlError::PasswordNotFound(label.to_string()))
        }

        pub(super) fn delete(_service: &str, _user: &str) -> Result<()> {
            Ok(())
        }
    }

    pub(super) fn set(service: &str, user: &str, password: &str) -> Result<()> {
        if let Some(e) = forced_failure() {
            return Err(e);
        }
        if opted_out_of_keyring() {
            return Ok(());
        }
        backend::set(service, user, password)
    }

    pub(super) fn get(service: &str, user: &str, label: &str) -> Result<String> {
        if let Some(e) = forced_failure() {
            return Err(e);
        }
        if opted_out_of_keyring() {
            return Err(SbqlError::PasswordNotFound(label.to_string()));
        }
        backend::get(service, user, label)
    }

    pub(super) fn delete(service: &str, user: &str) -> Result<()> {
        if let Some(e) = forced_failure() {
            return Err(e);
        }
        if opted_out_of_keyring() {
            return Ok(());
        }
        backend::delete(service, user)
    }
}

/// SSL connection mode for PostgreSQL.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SslMode {
    #[default]
    Prefer,
    Disable,
    Require,
    VerifyCa,
    VerifyFull,
}

impl SslMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            SslMode::Disable => "disable",
            SslMode::Prefer => "prefer",
            SslMode::Require => "require",
            SslMode::VerifyCa => "verify-ca",
            SslMode::VerifyFull => "verify-full",
        }
    }
}

/// A saved database connection. Passwords are stored in the OS keyring,
/// never inside this struct or on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub backend: DbBackend,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub database: String,
    #[serde(default)]
    pub ssl_mode: SslMode,
    /// File path for SQLite databases (only used when `backend == Sqlite`).
    #[serde(default)]
    pub file_path: Option<String>,
    /// Whether SSH tunneling is enabled for this connection.
    #[serde(default)]
    pub ssh_enabled: bool,
    /// SSH server hostname.
    #[serde(default)]
    pub ssh_host: String,
    /// SSH server port.
    #[serde(default = "default_ssh_port")]
    pub ssh_port: u16,
    /// SSH username.
    #[serde(default)]
    pub ssh_user: String,
    /// SSH authentication method: "password" or "key".
    #[serde(default)]
    pub ssh_auth_method: String,
    /// Path to SSH private key file (used when `ssh_auth_method == "key"`).
    #[serde(default)]
    pub ssh_key_path: Option<String>,
}

fn default_ssh_port() -> u16 {
    22
}

impl ConnectionConfig {
    /// Create a new PostgreSQL connection config.
    pub fn new_postgres(
        name: impl Into<String>,
        host: impl Into<String>,
        port: u16,
        user: impl Into<String>,
        database: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            backend: DbBackend::Postgres,
            host: host.into(),
            port,
            user: user.into(),
            database: database.into(),
            ssl_mode: SslMode::Prefer,
            file_path: None,
            ssh_enabled: false,
            ssh_host: String::new(),
            ssh_port: 22,
            ssh_user: String::new(),
            ssh_auth_method: String::new(),
            ssh_key_path: None,
        }
    }

    /// Create a new SQLite connection config.
    pub fn new_sqlite(name: impl Into<String>, file_path: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            backend: DbBackend::Sqlite,
            host: String::new(),
            port: 0,
            user: String::new(),
            database: String::new(),
            ssl_mode: SslMode::default(),
            file_path: Some(file_path.into()),
            ssh_enabled: false,
            ssh_host: String::new(),
            ssh_port: 22,
            ssh_user: String::new(),
            ssh_auth_method: String::new(),
            ssh_key_path: None,
        }
    }

    /// Create a new MySQL connection config.
    pub fn new_mysql(
        name: impl Into<String>,
        host: impl Into<String>,
        port: u16,
        user: impl Into<String>,
        database: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            backend: DbBackend::Mysql,
            host: host.into(),
            port,
            user: user.into(),
            database: database.into(),
            ssl_mode: SslMode::default(),
            file_path: None,
            ssh_enabled: false,
            ssh_host: String::new(),
            ssh_port: 22,
            ssh_user: String::new(),
            ssh_auth_method: String::new(),
            ssh_key_path: None,
        }
    }

    /// Create a new DynamoDB connection config.
    ///
    /// `region` is stored in the `database` field, `host`/`port` form the
    /// optional endpoint override, `user` holds the AWS access-key id (secret
    /// key goes into the keyring).
    pub fn new_dynamodb(
        name: impl Into<String>,
        host: impl Into<String>,
        port: u16,
        region: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            backend: DbBackend::DynamoDb,
            host: host.into(),
            port,
            user: String::new(),
            database: region.into(),
            ssl_mode: SslMode::default(),
            file_path: None,
            ssh_enabled: false,
            ssh_host: String::new(),
            ssh_port: 22,
            ssh_user: String::new(),
            ssh_auth_method: String::new(),
            ssh_key_path: None,
        }
    }

    /// Create a new MongoDB connection config.
    pub fn new_mongodb(
        name: impl Into<String>,
        host: impl Into<String>,
        port: u16,
        database: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            backend: DbBackend::MongoDb,
            host: host.into(),
            port,
            user: String::new(),
            database: database.into(),
            ssl_mode: SslMode::default(),
            file_path: None,
            ssh_enabled: false,
            ssh_host: String::new(),
            ssh_port: 22,
            ssh_user: String::new(),
            ssh_auth_method: String::new(),
            ssh_key_path: None,
        }
    }

    /// Create a new SQL Server connection config.
    pub fn new_sqlserver(
        name: impl Into<String>,
        host: impl Into<String>,
        port: u16,
        user: impl Into<String>,
        database: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            backend: DbBackend::SqlServer,
            host: host.into(),
            port,
            user: user.into(),
            database: database.into(),
            ssl_mode: SslMode::default(),
            file_path: None,
            ssh_enabled: false,
            ssh_host: String::new(),
            ssh_port: 22,
            ssh_user: String::new(),
            ssh_auth_method: String::new(),
            ssh_key_path: None,
        }
    }

    /// Create a new Redis connection config.
    pub fn new_redis(name: impl Into<String>, host: impl Into<String>, port: u16) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            backend: DbBackend::Redis,
            host: host.into(),
            port,
            user: String::new(),
            database: "0".to_string(),
            ssl_mode: SslMode::default(),
            file_path: None,
            ssh_enabled: false,
            ssh_host: String::new(),
            ssh_port: 22,
            ssh_user: String::new(),
            ssh_auth_method: String::new(),
            ssh_key_path: None,
        }
    }

    /// Build the connection string appropriate for this backend.
    pub fn connection_string(&self, password: &str) -> String {
        // Every userinfo/path segment is percent-encoded, not just the
        // password: a user like `domain\user` or a database name with a space
        // otherwise produced a malformed URL that the driver rejected.
        let user = urlencoding_simple(&self.user);
        let pass = urlencoding_simple(password);
        let db = urlencoding_simple(&self.database);
        match self.backend {
            DbBackend::Postgres => format!(
                "postgresql://{}:{}@{}:{}/{}?sslmode={}",
                user,
                pass,
                self.host,
                self.port,
                db,
                self.ssl_mode.as_str(),
            ),
            DbBackend::Sqlite => {
                let path = self.file_path.as_deref().unwrap_or(":memory:");
                format!("sqlite:{path}")
            }
            DbBackend::Mysql => format!(
                "mysql://{}:{}@{}:{}/{}",
                user, pass, self.host, self.port, db,
            ),
            DbBackend::Redis => {
                let scheme = if self.ssl_mode == SslMode::Require {
                    "rediss"
                } else {
                    "redis"
                };
                if !self.user.is_empty() || !password.is_empty() {
                    format!(
                        "{scheme}://{}:{}@{}:{}/{}",
                        user, pass, self.host, self.port, db,
                    )
                } else {
                    format!("{scheme}://{}:{}/{}", self.host, self.port, db,)
                }
            }
            DbBackend::DynamoDb => format!("http://{}:{}", self.host, self.port),
            DbBackend::MongoDb => {
                if !self.user.is_empty() || !password.is_empty() {
                    format!(
                        "mongodb://{}:{}@{}:{}/{}",
                        user, pass, self.host, self.port, db,
                    )
                } else {
                    format!("mongodb://{}:{}/{}", self.host, self.port, db,)
                }
            }
            DbBackend::SqlServer => {
                format!(
                    "sqlserver://{}:{}@{}:{}/{}",
                    user, pass, self.host, self.port, db,
                )
            }
        }
    }

    /// Keyring key for this connection's password.
    fn keyring_user(&self) -> String {
        format!("sbql/{}", self.id)
    }

    /// Store the password in the OS keyring. No-op for SQLite.
    pub fn save_password(&self, password: &str) -> Result<()> {
        if self.backend == DbBackend::Sqlite
            || (self.backend == DbBackend::Redis && password.is_empty())
            || (self.backend == DbBackend::DynamoDb && password.is_empty())
            || (self.backend == DbBackend::MongoDb && password.is_empty())
        {
            return Ok(());
        }
        store::set(store::SERVICE, &self.keyring_user(), password)
    }

    /// Retrieve the password from the OS keyring. Returns empty string for SQLite.
    pub fn load_password(&self) -> Result<String> {
        if self.backend == DbBackend::Sqlite {
            return Ok(String::new());
        }
        store::get(store::SERVICE, &self.keyring_user(), &self.name)
    }

    /// Delete the password from the OS keyring. No-op for SQLite.
    pub fn delete_password(&self) -> Result<()> {
        if self.backend == DbBackend::Sqlite {
            return Ok(());
        }
        store::delete(store::SERVICE, &self.keyring_user())
    }

    /// Store the SSH password in the OS keyring.
    pub fn save_ssh_password(&self, password: &str) -> Result<()> {
        if password.is_empty() {
            return Ok(());
        }
        store::set(store::SSH_SERVICE, &self.id.to_string(), password)
    }

    /// Retrieve the SSH password from the OS keyring.
    ///
    /// Returns an empty string when nothing is stored — but a store that is
    /// present-yet-broken is logged rather than silently swallowed, so an SSH
    /// auth failure that is really an unreadable keyring is diagnosable
    /// instead of looking like a wrong password.
    pub fn load_ssh_password(&self) -> String {
        match store::get(store::SSH_SERVICE, &self.id.to_string(), &self.name) {
            Ok(pw) => pw,
            Err(SbqlError::PasswordNotFound(_)) => String::new(),
            Err(e) => {
                tracing::warn!(
                    "SSH password store unreadable for '{}': {e} — proceeding with no password",
                    self.name
                );
                String::new()
            }
        }
    }

    /// Whether `other` connects to the same place, the same way.
    ///
    /// Compares every field that feeds the pool or the SSH tunnel — everything
    /// except `id` and `name`. A live pool built from a config for which this
    /// returns false is stale and must not be reused.
    pub fn same_target(&self, other: &ConnectionConfig) -> bool {
        self.backend == other.backend
            && self.host == other.host
            && self.port == other.port
            && self.user == other.user
            && self.database == other.database
            && self.ssl_mode == other.ssl_mode
            && self.file_path == other.file_path
            && self.ssh_enabled == other.ssh_enabled
            && self.ssh_host == other.ssh_host
            && self.ssh_port == other.ssh_port
            && self.ssh_user == other.ssh_user
            && self.ssh_auth_method == other.ssh_auth_method
            && self.ssh_key_path == other.ssh_key_path
    }
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Default)]
struct ConfigFile {
    connections: Vec<ConnectionConfig>,
}

/// Returns `~/.config/sbql/connections.toml`, creating parent dirs if needed.
pub fn config_path() -> Result<PathBuf> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("connections.toml"))
}

/// Directory holding the connection file.
///
/// `SBQL_CONFIG_DIR` overrides it — the test suite relies on that so it never
/// overwrites the developer's real connections, and it lets users keep several
/// profiles side by side.
fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os(CONFIG_DIR_ENV) {
        return PathBuf::from(dir);
    }
    let base = dirs::config_dir().unwrap_or_else(|| {
        PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config")
    });
    base.join("sbql")
}

/// Load all saved connections from disk.
pub fn load_connections() -> Result<Vec<ConnectionConfig>> {
    let path = config_path()?;
    load_connections_from(&path)
}

/// Persist the full list of connections to disk (passwords are NOT written).
pub fn save_connections(connections: &[ConnectionConfig]) -> Result<()> {
    let path = config_path()?;
    save_connections_to(&path, connections)
}

/// Load connections from an arbitrary path (useful for testing).
pub fn load_connections_from(path: &std::path::Path) -> Result<Vec<ConnectionConfig>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(path)?;
    let cfg: ConfigFile =
        toml::from_str(&raw).map_err(|e| SbqlError::Serialization(e.to_string()))?;
    Ok(cfg.connections)
}

/// Persist connections to an arbitrary path (useful for testing).
///
/// The write is atomic: the TOML is written to a sibling temp file and renamed
/// over the target. A plain `std::fs::write` truncates first, so an interrupt
/// (crash, full disk, power loss) mid-write left a truncated or empty
/// `connections.toml` — every saved connection gone.
pub fn save_connections_to(path: &std::path::Path, connections: &[ConnectionConfig]) -> Result<()> {
    let cfg = ConfigFile {
        connections: connections.to_vec(),
    };
    let raw = toml::to_string_pretty(&cfg).map_err(|e| SbqlError::Serialization(e.to_string()))?;

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    // Same directory as the target so the rename stays on one filesystem
    // (a cross-device rename fails). The pid plus a process-wide counter keeps
    // concurrent writers on distinct temp files — a pid alone is shared by all
    // threads and let two saves clobber each other's temp.
    static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = path.with_extension(format!("toml.tmp.{}.{seq}", std::process::id()));
    std::fs::write(&tmp, raw)?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e.into())
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Minimal percent-encoding for special characters in a password that appear
/// in a URL-form connection string.
fn urlencoding_simple(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            // Characters that are safe inside a URL userinfo segment
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => out.push(ch),
            _ => {
                for byte in ch.to_string().as_bytes() {
                    out.push_str(&format!("%{:02X}", byte));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sslmode_as_str() {
        assert_eq!(SslMode::Prefer.as_str(), "prefer");
        assert_eq!(SslMode::Disable.as_str(), "disable");
        assert_eq!(SslMode::Require.as_str(), "require");
        assert_eq!(SslMode::VerifyCa.as_str(), "verify-ca");
        assert_eq!(SslMode::VerifyFull.as_str(), "verify-full");
    }

    #[test]
    fn test_connection_config_new() {
        let conn =
            ConnectionConfig::new_postgres("local", "localhost", 5432, "postgres", "postgres");
        assert_eq!(conn.name, "local");
        assert_eq!(conn.host, "localhost");
        assert_eq!(conn.port, 5432);
        assert_eq!(conn.user, "postgres");
        assert_eq!(conn.database, "postgres");
        assert_eq!(conn.ssl_mode, SslMode::Prefer);
    }

    #[test]
    fn test_connection_string() {
        let conn = ConnectionConfig::new_postgres("local", "localhost", 5432, "postgres", "mydb");
        let dsn = conn.connection_string("p@ssw/rd");
        assert_eq!(
            dsn,
            "postgresql://postgres:p%40ssw%2Frd@localhost:5432/mydb?sslmode=prefer"
        );
    }

    #[test]
    fn test_urlencoding_simple() {
        assert_eq!(urlencoding_simple("normal123"), "normal123");
        assert_eq!(urlencoding_simple("with space"), "with%20space");
        assert_eq!(urlencoding_simple("special@/#"), "special%40%2F%23");
        assert_eq!(urlencoding_simple("-_.~"), "-_.~"); // Unreserved characters
    }

    // -- File I/O tests --

    #[test]
    fn round_trip_save_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("connections.toml");

        let conns = vec![
            ConnectionConfig::new_postgres("test1", "host1", 5432, "user1", "db1"),
            ConnectionConfig::new_postgres("test2", "host2", 3333, "user2", "db2"),
        ];

        save_connections_to(&path, &conns).unwrap();
        let loaded = load_connections_from(&path).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "test1");
        assert_eq!(loaded[1].name, "test2");
        assert_eq!(loaded[0].port, 5432);
        assert_eq!(loaded[1].port, 3333);
    }

    #[test]
    fn load_missing_file_empty_vec() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.toml");
        let loaded = load_connections_from(&path).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn load_invalid_toml_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.toml");
        std::fs::write(&path, "this is not valid [[[ toml").unwrap();
        let result = load_connections_from(&path);
        assert!(result.is_err());
    }

    #[test]
    fn ssl_mode_serde_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ssl_test.toml");

        let mut conn = ConnectionConfig::new_postgres("ssl_test", "h", 5432, "u", "d");
        conn.ssl_mode = SslMode::VerifyFull;
        save_connections_to(&path, &[conn]).unwrap();

        let loaded = load_connections_from(&path).unwrap();
        assert_eq!(loaded[0].ssl_mode, SslMode::VerifyFull);
    }

    #[test]
    fn round_trip_sqlite_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sqlite_conns.toml");

        let conns = vec![
            ConnectionConfig::new_sqlite("my_sqlite", "/tmp/test.db"),
            ConnectionConfig::new_postgres("pg_conn", "localhost", 5432, "user", "db"),
        ];
        save_connections_to(&path, &conns).unwrap();
        let loaded = load_connections_from(&path).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].backend, DbBackend::Sqlite);
        assert_eq!(loaded[0].file_path, Some("/tmp/test.db".to_string()));
        assert_eq!(loaded[0].name, "my_sqlite");

        assert_eq!(loaded[1].backend, DbBackend::Postgres);
        assert!(loaded[1].file_path.is_none());
    }

    #[test]
    fn backward_compat_no_backend_field() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old_format.toml");

        // Simulate an old TOML file without `backend` or `file_path` fields
        let toml_content = r#"
[[connections]]
id = "00000000-0000-0000-0000-000000000001"
name = "legacy"
host = "localhost"
port = 5432
user = "postgres"
database = "mydb"
"#;
        std::fs::write(&path, toml_content).unwrap();
        let loaded = load_connections_from(&path).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].backend, DbBackend::Postgres); // default
        assert!(loaded[0].file_path.is_none()); // default
        assert_eq!(loaded[0].name, "legacy");
    }

    #[test]
    fn sqlite_connection_string() {
        let conn = ConnectionConfig::new_sqlite("test", "/data/app.db");
        assert_eq!(conn.connection_string(""), "sqlite:/data/app.db");
    }

    #[test]
    fn sqlite_new_sqlite_constructor() {
        let conn = ConnectionConfig::new_sqlite("mydb", "/tmp/test.sqlite");
        assert_eq!(conn.backend, DbBackend::Sqlite);
        assert_eq!(conn.name, "mydb");
        assert_eq!(conn.file_path, Some("/tmp/test.sqlite".to_string()));
        assert!(conn.host.is_empty());
        assert_eq!(conn.port, 0);
    }

    // --- Phase 1C: additional gap tests ---

    #[test]
    fn sqlite_connection_string_memory_when_no_file_path() {
        let mut conn = ConnectionConfig::new_sqlite("mem", "");
        conn.file_path = None;
        assert_eq!(conn.connection_string(""), "sqlite::memory:");
    }

    #[test]
    fn sqlite_password_ops_are_noop() {
        let conn = ConnectionConfig::new_sqlite("test", "/tmp/test.db");
        // save_password should succeed (no-op)
        assert!(conn.save_password("secret").is_ok());
        // load_password should return empty string
        assert_eq!(conn.load_password().unwrap(), "");
        // delete_password should succeed (no-op)
        assert!(conn.delete_password().is_ok());
    }

    #[test]
    fn urlencoding_multibyte_utf8() {
        // Test with multi-byte UTF-8 characters (e.g. emoji, CJK)
        let encoded = urlencoding_simple("café");
        assert!(encoded.starts_with("caf"));
        // 'é' is U+00E9, 2 bytes: 0xC3 0xA9
        assert!(encoded.contains("%C3%A9"));
    }

    #[test]
    fn urlencoding_emoji() {
        let encoded = urlencoding_simple("p@ss🔑");
        assert!(encoded.starts_with("p%40ss"));
        // Emoji should be percent-encoded as UTF-8 bytes
        assert!(encoded.contains("%F0"));
    }
}
