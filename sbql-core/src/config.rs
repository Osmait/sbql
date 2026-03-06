use std::path::PathBuf;

use keyring::Entry;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Result, SbqlError};

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
    pub host: String,
    pub port: u16,
    pub user: String,
    pub database: String,
    #[serde(default)]
    pub ssl_mode: SslMode,
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
            host: host.into(),
            port,
            user: user.into(),
            database: database.into(),
            ssl_mode: SslMode::Prefer,
        }
    }

    /// Build the libpq-style connection string (without the password).
    pub fn connection_string(&self, password: &str) -> String {
        format!(
            "postgresql://{}:{}@{}:{}/{}?sslmode={}",
            self.user,
            urlencoding_simple(password),
            self.host,
            self.port,
            self.database,
            self.ssl_mode.as_str(),
        )
    }

    /// Keyring key for this connection's password.
    fn keyring_user(&self) -> String {
        format!("sbql/{}", self.id)
    }

    /// Store the password in the OS keyring.
    pub fn save_password(&self, password: &str) -> Result<()> {
        let entry = Entry::new(KEYRING_SERVICE, &self.keyring_user())
            .map_err(|e| SbqlError::Keyring(e.to_string()))?;
        entry
            .set_password(password)
            .map_err(|e| SbqlError::Keyring(e.to_string()))
    }

    /// Retrieve the password from the OS keyring.
    pub fn load_password(&self) -> Result<String> {
        let entry = Entry::new(KEYRING_SERVICE, &self.keyring_user())
            .map_err(|e| SbqlError::Keyring(e.to_string()))?;
        entry.get_password().map_err(|e| {
            SbqlError::Keyring(format!("password not found for '{}': {}", self.name, e))
        })
    }

    /// Delete the password from the OS keyring.
    pub fn delete_password(&self) -> Result<()> {
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
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path)?;
    let cfg: ConfigFile =
        toml::from_str(&raw).map_err(|e| SbqlError::Serialization(e.to_string()))?;
    Ok(cfg.connections)
}

/// Persist the full list of connections to disk (passwords are NOT written).
pub fn save_connections(connections: &[ConnectionConfig]) -> Result<()> {
    let path = config_path()?;
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
