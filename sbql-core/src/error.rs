use thiserror::Error;

#[derive(Debug, Error)]
pub enum SbqlError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Keyring error: {0}")]
    Keyring(String),

    #[error("SQL parse error: {0}")]
    SqlParse(String),

    #[error("Connection not found: {0}")]
    ConnectionNotFound(String),

    #[error("No active connection")]
    NoActiveConnection,

    #[error("Schema introspection error: {0}")]
    Schema(String),

    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),
}

pub type Result<T> = std::result::Result<T, SbqlError>;
