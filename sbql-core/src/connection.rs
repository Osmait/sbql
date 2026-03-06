use std::collections::HashMap;
use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::ConnectionConfig;
use crate::error::{Result, SbqlError};

/// Manages a map of live `PgPool` instances keyed by connection id.
#[derive(Clone, Default)]
pub struct ConnectionManager {
    pools: Arc<RwLock<HashMap<Uuid, PgPool>>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open (or reuse) a connection pool, loading the password from keyring.
    #[allow(dead_code)]
    pub async fn connect(&self, config: &ConnectionConfig) -> Result<()> {
        let password = config.load_password()?;
        self.connect_with_password(config, &password).await
    }

    /// Open (or reuse) a connection pool with an explicit password.
    pub async fn connect_with_password(&self, config: &ConnectionConfig, password: &str) -> Result<()> {
        // Already connected?
        {
            let guard = self.pools.read().await;
            if guard.contains_key(&config.id) {
                return Ok(());
            }
        }

        let url = config.connection_string(password);

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(10))
            .connect(&url)
            .await?;

        self.pools.write().await.insert(config.id, pool);
        tracing::info!("Connected to '{}' ({})", config.name, config.id);
        Ok(())
    }

    /// Ping a connection by running `SELECT 1`.
    pub async fn ping(&self, id: Uuid) -> Result<()> {
        let guard = self.pools.read().await;
        let pool = guard
            .get(&id)
            .ok_or_else(|| SbqlError::ConnectionNotFound(id.to_string()))?;
        sqlx::query("SELECT 1").execute(pool).await?;
        Ok(())
    }

    /// Get a clone of the pool for the given connection id.
    pub async fn get(&self, id: Uuid) -> Result<PgPool> {
        let guard = self.pools.read().await;
        guard
            .get(&id)
            .cloned()
            .ok_or_else(|| SbqlError::ConnectionNotFound(id.to_string()))
    }

    /// Close and remove a pool.
    pub async fn disconnect(&self, id: Uuid) {
        if let Some(pool) = self.pools.write().await.remove(&id) {
            pool.close().await;
            tracing::info!("Disconnected {}", id);
        }
    }

    /// Returns the ids of all currently open connections.
    pub async fn active_ids(&self) -> Vec<Uuid> {
        self.pools.read().await.keys().copied().collect()
    }
}
