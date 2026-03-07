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
}

/// A pool that wraps either PostgreSQL or SQLite.
#[derive(Debug, Clone)]
pub enum DbPool {
    Postgres(PgPool),
    Sqlite(SqlitePool),
}

impl DbPool {
    /// Which backend this pool targets.
    pub fn backend(&self) -> DbBackend {
        match self {
            DbPool::Postgres(_) => DbBackend::Postgres,
            DbPool::Sqlite(_) => DbBackend::Sqlite,
        }
    }

    /// Gracefully shut down the pool.
    pub async fn close(&self) {
        match self {
            DbPool::Postgres(p) => p.close().await,
            DbPool::Sqlite(p) => p.close().await,
        }
    }
}
