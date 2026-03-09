use std::path::PathBuf;

use keyring::Entry;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Result, SbqlError};
use crate::pool::DbBackend;

const KEYRING_SERVICE: &str = "sbql";

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
}

impl ConnectionConfig {
    pub fn new(
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
        }
    }

    /// Build the connection string appropriate for this backend.
    pub fn connection_string(&self, password: &str) -> String {
        match self.backend {
            DbBackend::Postgres => format!(
                "postgresql://{}:{}@{}:{}/{}?sslmode={}",
                self.user,
                urlencoding_simple(password),
                self.host,
                self.port,
                self.database,
                self.ssl_mode.as_str(),
            ),
            DbBackend::Sqlite => {
                let path = self.file_path.as_deref().unwrap_or(":memory:");
                format!("sqlite:{path}")
            }
            DbBackend::Redis => {
                let scheme = if self.ssl_mode == SslMode::Require {
                    "rediss"
                } else {
                    "redis"
                };
                if !self.user.is_empty() || !password.is_empty() {
                    format!(
                        "{scheme}://{}:{}@{}:{}/{}",
                        self.user,
                        urlencoding_simple(password),
                        self.host,
                        self.port,
                        self.database,
                    )
                } else {
                    format!("{scheme}://{}:{}/{}", self.host, self.port, self.database,)
                }
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
        {
            return Ok(());
        }
        let entry = Entry::new(KEYRING_SERVICE, &self.keyring_user())
            .map_err(|e| SbqlError::Keyring(e.to_string()))?;
        entry
            .set_password(password)
            .map_err(|e| SbqlError::Keyring(e.to_string()))
    }

    /// Retrieve the password from the OS keyring. Returns empty string for SQLite.
    pub fn load_password(&self) -> Result<String> {
        if self.backend == DbBackend::Sqlite {
            return Ok(String::new());
        }
        let entry = Entry::new(KEYRING_SERVICE, &self.keyring_user())
            .map_err(|e| SbqlError::Keyring(e.to_string()))?;
        entry.get_password().map_err(|e| {
            SbqlError::Keyring(format!("password not found for '{}': {}", self.name, e))
        })
    }

    /// Delete the password from the OS keyring. No-op for SQLite.
    pub fn delete_password(&self) -> Result<()> {
        if self.backend == DbBackend::Sqlite {
            return Ok(());
        }
        let entry = Entry::new(KEYRING_SERVICE, &self.keyring_user())
            .map_err(|e| SbqlError::Keyring(e.to_string()))?;
        entry
            .delete_credential()
            .map_err(|e| SbqlError::Keyring(e.to_string()))
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
    let base = dirs::config_dir().unwrap_or_else(|| {
        PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config")
    });
    let dir = base.join("sbql");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("connections.toml"))
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
pub fn save_connections_to(path: &std::path::Path, connections: &[ConnectionConfig]) -> Result<()> {
    let cfg = ConfigFile {
        connections: connections.to_vec(),
    };
    let raw = toml::to_string_pretty(&cfg).map_err(|e| SbqlError::Serialization(e.to_string()))?;
    std::fs::write(path, raw)?;
    Ok(())
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
        let conn = ConnectionConfig::new("local", "localhost", 5432, "postgres", "postgres");
        assert_eq!(conn.name, "local");
        assert_eq!(conn.host, "localhost");
        assert_eq!(conn.port, 5432);
        assert_eq!(conn.user, "postgres");
        assert_eq!(conn.database, "postgres");
        assert_eq!(conn.ssl_mode, SslMode::Prefer);
    }

    #[test]
    fn test_connection_string() {
        let conn = ConnectionConfig::new("local", "localhost", 5432, "postgres", "mydb");
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
            ConnectionConfig::new("test1", "host1", 5432, "user1", "db1"),
            ConnectionConfig::new("test2", "host2", 3333, "user2", "db2"),
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

        let mut conn = ConnectionConfig::new("ssl_test", "h", 5432, "u", "d");
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
            ConnectionConfig::new("pg_conn", "localhost", 5432, "user", "db"),
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
