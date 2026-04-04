use std::collections::HashMap;
use std::sync::Arc;

use sqlx::mysql::MySqlPoolOptions;
use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::SqlitePoolOptions;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::ConnectionConfig;
use crate::error::{Result, SbqlError};
use crate::pool::{DbBackend, DbPool};

/// Manages a map of live [`DbPool`] instances keyed by connection id.
#[derive(Clone, Default)]
pub struct ConnectionManager {
    pools: Arc<RwLock<HashMap<Uuid, DbPool>>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open (or reuse) a connection pool with an explicit password.
    #[tracing::instrument(skip_all, fields(name = config.name, backend = ?config.backend))]
    pub async fn connect_with_password(
        &self,
        config: &ConnectionConfig,
        password: &str,
    ) -> Result<()> {
        // Already connected?
        {
            let guard = self.pools.read().await;
            if guard.contains_key(&config.id) {
                return Ok(());
            }
        }

        let url = config.connection_string(password);

        let pool = match config.backend {
            DbBackend::Postgres => {
                let pg = PgPoolOptions::new()
                    .max_connections(5)
                    .acquire_timeout(std::time::Duration::from_secs(10))
                    .connect(&url)
                    .await?;
                DbPool::Postgres(pg)
            }
            DbBackend::Sqlite => {
                let sq = SqlitePoolOptions::new()
                    .max_connections(5)
                    .acquire_timeout(std::time::Duration::from_secs(10))
                    .after_connect(|conn, _meta| {
                        Box::pin(async move {
                            sqlx::query("PRAGMA foreign_keys = ON")
                                .execute(&mut *conn)
                                .await?;
                            Ok(())
                        })
                    })
                    .connect(&url)
                    .await?;
                DbPool::Sqlite(sq)
            }
            DbBackend::Mysql => {
                let my = MySqlPoolOptions::new()
                    .max_connections(5)
                    .acquire_timeout(std::time::Duration::from_secs(10))
                    .connect(&url)
                    .await?;
                DbPool::Mysql(my)
            }
            DbBackend::Redis => {
                let client = redis::Client::open(url.as_str())?;
                let cm = redis::aio::ConnectionManager::new(client).await?;
                DbPool::Redis(Box::new(cm))
            }
            DbBackend::DynamoDb => {
                let endpoint = url.clone();
                let region = config.database.clone();
                let access_key = config.user.clone();
                let secret_key = password.to_string();

                let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest())
                    .region(aws_config::Region::new(region));

                if !access_key.is_empty() && !secret_key.is_empty() {
                    loader = loader.credentials_provider(
                        aws_sdk_dynamodb::config::Credentials::new(
                            access_key,
                            secret_key,
                            None,
                            None,
                            "sbql",
                        ),
                    );
                }

                let sdk_config = loader.load().await;
                let mut dynamo_config =
                    aws_sdk_dynamodb::config::Builder::from(&sdk_config);

                if !endpoint.is_empty() && endpoint != "http://:0" {
                    dynamo_config = dynamo_config.endpoint_url(&endpoint);
                }

                let client =
                    aws_sdk_dynamodb::Client::from_conf(dynamo_config.build());
                DbPool::DynamoDb(Box::new(client))
            }
            DbBackend::MongoDb => {
                let client_options = mongodb::options::ClientOptions::parse(&url)
                    .await
                    .map_err(|e| SbqlError::MongoDb(e.to_string()))?;
                let client = mongodb::Client::with_options(client_options)
                    .map_err(|e| SbqlError::MongoDb(e.to_string()))?;
                let db = client.database(&config.database);
                DbPool::MongoDb(Box::new(db))
            }
        };

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
        match pool {
            DbPool::Postgres(pg) => {
                sqlx::query("SELECT 1").execute(pg).await?;
            }
            DbPool::Sqlite(sq) => {
                sqlx::query("SELECT 1").execute(sq).await?;
            }
            DbPool::Mysql(my) => {
                sqlx::query("SELECT 1").execute(my).await?;
            }
            DbPool::Redis(cm) => {
                let mut conn = cm.as_ref().clone();
                let _: String = redis::cmd("PING").query_async(&mut conn).await?;
            }
            DbPool::DynamoDb(client) => {
                client
                    .list_tables()
                    .limit(1)
                    .send()
                    .await
                    .map_err(|e| SbqlError::DynamoDb(e.to_string()))?;
            }
            DbPool::MongoDb(db) => {
                db.run_command(mongodb::bson::doc! { "ping": 1 })
                    .await
                    .map_err(|e| SbqlError::MongoDb(e.to_string()))?;
            }
        }
        Ok(())
    }

    /// Get a clone of the pool for the given connection id.
    pub async fn get(&self, id: Uuid) -> Result<DbPool> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_manager_initialization() {
        let manager = ConnectionManager::new();
        let ids = manager.active_ids().await;
        assert!(ids.is_empty());
    }

    #[tokio::test]
    async fn test_manager_get_missing_pool() {
        let manager = ConnectionManager::new();
        let id = Uuid::new_v4();

        let result = manager.get(id).await;
        assert!(result.is_err());
        if let Err(SbqlError::ConnectionNotFound(msg)) = result {
            assert_eq!(msg, id.to_string());
        } else {
            panic!("Expected ConnectionNotFound error");
        }
    }

    #[tokio::test]
    async fn test_manager_ping_missing_pool() {
        let manager = ConnectionManager::new();
        let id = Uuid::new_v4();

        let result = manager.ping(id).await;
        assert!(result.is_err());
        if let Err(SbqlError::ConnectionNotFound(msg)) = result {
            assert_eq!(msg, id.to_string());
        } else {
            panic!("Expected ConnectionNotFound error");
        }
    }

    #[tokio::test]
    async fn test_manager_disconnect_missing_pool() {
        let manager = ConnectionManager::new();
        let id = Uuid::new_v4();

        // This should just silently return without doing anything or crashing
        manager.disconnect(id).await;
        let ids = manager.active_ids().await;
        assert!(ids.is_empty());
    }

    // --- Phase 1D: live SQLite tests ---

    #[tokio::test]
    async fn test_connect_sqlite_in_memory() {
        let manager = ConnectionManager::new();
        let config = ConnectionConfig::new_sqlite("test", ":memory:");
        manager
            .connect_with_password(&config, "")
            .await
            .expect("should connect to SQLite in-memory");
        let ids = manager.active_ids().await;
        assert!(ids.contains(&config.id));
    }

    #[tokio::test]
    async fn test_connect_twice_is_idempotent() {
        let manager = ConnectionManager::new();
        let config = ConnectionConfig::new_sqlite("test", ":memory:");
        manager
            .connect_with_password(&config, "")
            .await
            .expect("first connect");
        // Second connect with same id should be a no-op (early return)
        manager
            .connect_with_password(&config, "")
            .await
            .expect("second connect should succeed");
        let ids = manager.active_ids().await;
        assert_eq!(ids.len(), 1);
    }

    #[tokio::test]
    async fn test_ping_sqlite_pool() {
        let manager = ConnectionManager::new();
        let config = ConnectionConfig::new_sqlite("test", ":memory:");
        manager.connect_with_password(&config, "").await.unwrap();
        manager
            .ping(config.id)
            .await
            .expect("ping should succeed on live pool");
    }

    #[tokio::test]
    async fn test_disconnect_removes_pool() {
        let manager = ConnectionManager::new();
        let config = ConnectionConfig::new_sqlite("test", ":memory:");
        manager.connect_with_password(&config, "").await.unwrap();
        assert!(!manager.active_ids().await.is_empty());

        manager.disconnect(config.id).await;
        assert!(manager.active_ids().await.is_empty());

        // get after disconnect should fail
        assert!(manager.get(config.id).await.is_err());
    }

    #[tokio::test]
    async fn test_connect_invalid_connection_string() {
        let manager = ConnectionManager::new();
        let mut config = ConnectionConfig::new_sqlite("bad", "");
        // Provide an invalid file path that doesn't exist and can't be created
        config.file_path = Some("/nonexistent/directory/that/does/not/exist/test.db".to_string());
        let result = manager.connect_with_password(&config, "").await;
        assert!(result.is_err());
    }
}
