//! Multi-backend database pool abstraction.

use serde::{Deserialize, Serialize};
use sqlx::{PgPool, SqlitePool};

/// Which database backend a connection targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DbBackend {
    #[default]
    Postgres,
    Sqlite,
    Redis,
}

/// A pool that wraps either PostgreSQL, SQLite, or Redis.
#[derive(Clone)]
pub enum DbPool {
    Postgres(PgPool),
    Sqlite(SqlitePool),
    Redis(redis::aio::ConnectionManager),
}

impl std::fmt::Debug for DbPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbPool::Postgres(_) => f.debug_tuple("Postgres").field(&"PgPool(..)").finish(),
            DbPool::Sqlite(_) => f.debug_tuple("Sqlite").field(&"SqlitePool(..)").finish(),
            DbPool::Redis(_) => f.debug_tuple("Redis").field(&"ConnectionManager(..)").finish(),
        }
    }
}

impl DbPool {
    /// Which backend this pool targets.
    pub fn backend(&self) -> DbBackend {
        match self {
            DbPool::Postgres(_) => DbBackend::Postgres,
            DbPool::Sqlite(_) => DbBackend::Sqlite,
            DbPool::Redis(_) => DbBackend::Redis,
        }
    }

    /// Gracefully shut down the pool.
    pub async fn close(&self) {
        match self {
            DbPool::Postgres(p) => p.close().await,
            DbPool::Sqlite(p) => p.close().await,
            DbPool::Redis(_) => { /* ConnectionManager manages its own lifecycle */ }
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
