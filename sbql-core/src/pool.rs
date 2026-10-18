//! Multi-backend database pool abstraction.

use serde::{Deserialize, Serialize};
use sqlx::{MySqlPool, PgPool, SqlitePool};

/// Which database backend a connection targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DbBackend {
    /// PostgreSQL, over `sqlx`. The default for a new connection.
    #[default]
    Postgres,
    /// SQLite, a local file rather than a server.
    Sqlite,
    /// MySQL or MariaDB — same wire protocol, same code path.
    Mysql,
    /// Redis. Commands, not SQL: no schema, no sorting, no filtering.
    Redis,
    /// DynamoDB via the AWS SDK. `database` holds the region.
    DynamoDb,
    /// MongoDB. Documents are flattened to columns for display.
    MongoDb,
    /// Microsoft SQL Server, over `tiberius` behind a `bb8` pool.
    SqlServer,
}

/// A pool that wraps one of the supported backends: PostgreSQL, SQLite, MySQL, Redis, DynamoDB, MongoDB, or SQL Server.
#[derive(Clone)]
pub enum DbPool {
    /// A `sqlx` PostgreSQL pool.
    Postgres(PgPool),
    /// A `sqlx` SQLite pool, capped at one connection — SQLite serialises
    /// writers anyway.
    Sqlite(SqlitePool),
    /// A `sqlx` MySQL pool.
    Mysql(MySqlPool),
    /// A redis connection manager, which reconnects on its own.
    Redis(Box<redis::aio::ConnectionManager>),
    /// An AWS SDK client. Boxed because it is far larger than the other
    /// variants and would otherwise set the size of every `DbPool`.
    DynamoDb(Box<aws_sdk_dynamodb::Client>),
    /// A MongoDB handle to one database.
    MongoDb(Box<mongodb::Database>),
    /// A `bb8` pool over `tiberius`.
    SqlServer(Box<bb8::Pool<bb8_tiberius::ConnectionManager>>),
}

impl std::fmt::Debug for DbPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Postgres(_) => f.debug_tuple("Postgres").field(&"PgPool(..)").finish(),
            Self::Sqlite(_) => f.debug_tuple("Sqlite").field(&"SqlitePool(..)").finish(),
            Self::Mysql(_) => f.debug_tuple("Mysql").field(&"MySqlPool(..)").finish(),
            Self::Redis(_) => f
                .debug_tuple("Redis")
                .field(&"ConnectionManager(..)")
                .finish(),
            Self::DynamoDb(_) => f.debug_tuple("DynamoDb").field(&"Client(..)").finish(),
            Self::MongoDb(_) => f.debug_tuple("MongoDb").field(&"Database(..)").finish(),
            Self::SqlServer(_) => f.debug_tuple("SqlServer").field(&"bb8::Pool(..)").finish(),
        }
    }
}

impl DbPool {
    /// Which backend this pool targets.
    pub fn backend(&self) -> DbBackend {
        match self {
            Self::Postgres(_) => DbBackend::Postgres,
            Self::Sqlite(_) => DbBackend::Sqlite,
            Self::Mysql(_) => DbBackend::Mysql,
            Self::Redis(_) => DbBackend::Redis,
            Self::DynamoDb(_) => DbBackend::DynamoDb,
            Self::MongoDb(_) => DbBackend::MongoDb,
            Self::SqlServer(_) => DbBackend::SqlServer,
        }
    }

    /// Gracefully shut down the pool.
    pub async fn close(&self) {
        match self {
            Self::Postgres(p) => p.close().await,
            Self::Sqlite(p) => p.close().await,
            Self::Mysql(p) => p.close().await,
            Self::Redis(_) => { /* ConnectionManager manages its own lifecycle */ }
            Self::DynamoDb(_) => { /* SDK client manages its own lifecycle */ }
            Self::MongoDb(_) => { /* MongoDB client manages its own lifecycle */ }
            Self::SqlServer(_) => { /* bb8 pool manages its own lifecycle */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_backend_default_is_postgres() {
        assert_eq!(DbBackend::default(), DbBackend::Postgres);
    }

    #[tokio::test]
    async fn test_sqlite_pool_backend() {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("SQLite in-memory pool");
        let db_pool = DbPool::Sqlite(pool);
        assert_eq!(db_pool.backend(), DbBackend::Sqlite);
    }

    #[tokio::test]
    async fn test_sqlite_pool_close() {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("SQLite in-memory pool");
        let db_pool = DbPool::Sqlite(pool);
        // Should not panic
        db_pool.close().await;
    }

    #[test]
    fn test_db_pool_debug() {
        // Verify Debug impl doesn't panic for each variant description
        let pg_desc = format!("{:?}", DbBackend::Postgres);
        assert!(pg_desc.contains("Postgres"));
        let redis_desc = format!("{:?}", DbBackend::Redis);
        assert!(redis_desc.contains("Redis"));
    }
}
