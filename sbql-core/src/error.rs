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

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Connection not found: {0}")]
    ConnectionNotFound(String),

    #[error("No active connection")]
    NoActiveConnection,

    #[error("Schema introspection error: {0}")]
    Schema(String),

    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("DynamoDB error: {0}")]
    DynamoDb(String),

    #[error("MongoDB error: {0}")]
    MongoDb(String),

    #[error("SQL Server error: {0}")]
    SqlServer(String),

    #[error("SSH tunnel error: {0}")]
    SshTunnel(String),

    #[error("Import error: {0}")]
    Import(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),
}

impl From<russh::Error> for SbqlError {
    fn from(e: russh::Error) -> Self {
        SbqlError::SshTunnel(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, SbqlError>;
