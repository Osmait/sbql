//! `sbql-ffi` — UniFFI bridge exposing `sbql-core` to Swift via C FFI.
//!
//! All public types use `#[derive(uniffi::Record)]` / `#[derive(uniffi::Enum)]`
//! and the engine object uses `#[derive(uniffi::Object)]` with exported methods.

mod convert;

use std::sync::Arc;
use tokio::sync::Mutex;

uniffi::setup_scaffolding!();

// ---------------------------------------------------------------------------
// FFI-safe enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiDbBackend {
    Postgres,
    Sqlite,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiSslMode {
    Prefer,
    Disable,
    Require,
    VerifyCa,
    VerifyFull,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiSortDirection {
    Ascending,
    Descending,
}

// ---------------------------------------------------------------------------
// FFI-safe records
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiConnectionConfig {
    pub id: String,
    pub name: String,
    pub backend: FfiDbBackend,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub database: String,
    pub ssl_mode: FfiSslMode,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiQueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub page: u32,
    pub has_next_page: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiTableEntry {
    pub schema: String,
    pub name: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_pk: bool,
    pub is_nullable: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiTableSchema {
    pub schema: String,
    pub name: String,
    pub columns: Vec<FfiColumnInfo>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiForeignKey {
    pub from_schema: String,
    pub from_table: String,
    pub from_col: String,
    pub to_schema: String,
    pub to_table: String,
    pub to_col: String,
    pub constraint_name: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiDiagramData {
    pub tables: Vec<FfiTableSchema>,
    pub foreign_keys: Vec<FfiForeignKey>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiFilterSuggestions {
    pub items: Vec<String>,
    pub token: u64,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SbqlFfiError {
    #[error("Core error: {msg}")]
    Core { msg: String },
    #[error("Invalid argument: {msg}")]
    InvalidArgument { msg: String },
}

// Convenience constructors
impl SbqlFfiError {
    fn core(msg: impl Into<String>) -> Self {
        SbqlFfiError::Core { msg: msg.into() }
    }
}

// ---------------------------------------------------------------------------
// SbqlEngine — the FFI Object
// ---------------------------------------------------------------------------

#[derive(uniffi::Object)]
pub struct SbqlEngine {
    core: Arc<Mutex<sbql_core::Core>>,
    #[allow(dead_code)]
    runtime: Arc<tokio::runtime::Runtime>,
}

#[uniffi::export]
impl SbqlEngine {
    /// Create a new engine, loading saved connections from disk.
    #[uniffi::constructor]
    pub fn new() -> Self {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime"),
        );
        let core = Arc::new(Mutex::new(sbql_core::Core::new()));
        Self { core, runtime }
    }

    // -------------------------------------------------------------------
    // Connection management
    // -------------------------------------------------------------------

    /// Return the list of saved connections.
    pub fn get_connections(&self) -> Vec<FfiConnectionConfig> {
        // This is sync because Core::connections is just a Vec in memory.
        // We can't block_on inside an async context, so we use try_lock
        // and fall back to loading from disk.
        if let Ok(core) = self.core.try_lock() {
            core.connections.iter().cloned().map(Into::into).collect()
        } else {
            sbql_core::load_connections()
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect()
        }
    }

    /// Save (create or update) a connection config. Returns updated list.
    #[uniffi::method(async_runtime = "tokio")]
    pub async fn save_connection(
        &self,
        config: FfiConnectionConfig,
        password: Option<String>,
    ) -> Result<Vec<FfiConnectionConfig>, SbqlFfiError> {
        let core_config: sbql_core::ConnectionConfig = config.try_into()?;
        let mut core = self.core.lock().await;
        let events = core
            .handle(sbql_core::CoreCommand::SaveConnection {
                config: core_config,
                password,
            })
            .await;
        extract_connection_list(events)
    }

    /// Delete a connection by id. Returns updated list.
    #[uniffi::method(async_runtime = "tokio")]
    pub async fn delete_connection(
        &self,
        id: String,
    ) -> Result<Vec<FfiConnectionConfig>, SbqlFfiError> {
        let uuid = parse_uuid(&id)?;
        let mut core = self.core.lock().await;
        let events = core
            .handle(sbql_core::CoreCommand::DeleteConnection(uuid))
            .await;
        extract_connection_list(events)
    }

    /// Open a connection pool.
    #[uniffi::method(async_runtime = "tokio")]
    pub async fn connect(&self, id: String) -> Result<(), SbqlFfiError> {
        let uuid = parse_uuid(&id)?;
        let mut core = self.core.lock().await;
        let events = core.handle(sbql_core::CoreCommand::Connect(uuid)).await;
        check_for_error(events)
    }

    /// Close a connection pool.
    #[uniffi::method(async_runtime = "tokio")]
    pub async fn disconnect(&self, id: String) -> Result<(), SbqlFfiError> {
        let uuid = parse_uuid(&id)?;
        let mut core = self.core.lock().await;
        let events = core.handle(sbql_core::CoreCommand::Disconnect(uuid)).await;
        check_for_error(events)
    }

    // -------------------------------------------------------------------
    // Schema
    // -------------------------------------------------------------------

    /// List tables in the active connection.
    #[uniffi::method(async_runtime = "tokio")]
    pub async fn list_tables(&self) -> Result<Vec<FfiTableEntry>, SbqlFfiError> {
        let mut core = self.core.lock().await;
        let events = core.handle(sbql_core::CoreCommand::ListTables).await;
        for ev in &events {
            if let sbql_core::CoreEvent::Error(msg) = ev {
                return Err(SbqlFfiError::core(msg));
            }
        }
        for ev in events {
            if let sbql_core::CoreEvent::TableList(tables) = ev {
                return Ok(tables.into_iter().map(Into::into).collect());
            }
        }
        Ok(vec![])
    }

    /// Get primary key columns for a table.
    #[uniffi::method(async_runtime = "tokio")]
    pub async fn get_primary_keys(
        &self,
        schema: String,
        table: String,
    ) -> Result<Vec<String>, SbqlFfiError> {
        let mut core = self.core.lock().await;
        let events = core
            .handle(sbql_core::CoreCommand::GetPrimaryKeys { schema, table })
            .await;
        for ev in &events {
            if let sbql_core::CoreEvent::Error(msg) = ev {
                return Err(SbqlFfiError::core(msg));
            }
        }
        for ev in events {
            if let sbql_core::CoreEvent::PrimaryKeys { columns, .. } = ev {
                return Ok(columns);
            }
        }
        Ok(vec![])
    }

    /// Load all table schemas and FK relationships for diagram view.
    #[uniffi::method(async_runtime = "tokio")]
    pub async fn load_diagram(&self) -> Result<FfiDiagramData, SbqlFfiError> {
        let mut core = self.core.lock().await;
        let events = core.handle(sbql_core::CoreCommand::LoadDiagram).await;
        for ev in &events {
            if let sbql_core::CoreEvent::Error(msg) = ev {
                return Err(SbqlFfiError::core(msg));
            }
        }
        for ev in events {
            if let sbql_core::CoreEvent::DiagramLoaded(data) = ev {
                return Ok(data.into());
            }
        }
        Ok(FfiDiagramData {
            tables: vec![],
            foreign_keys: vec![],
        })
    }

    // -------------------------------------------------------------------
    // Query
    // -------------------------------------------------------------------

    /// Execute a SQL query, returning page 0.
    #[uniffi::method(async_runtime = "tokio")]
    pub async fn execute_query(
        &self,
        sql: String,
    ) -> Result<FfiQueryResult, SbqlFfiError> {
        let mut core = self.core.lock().await;
        let events = core
            .handle(sbql_core::CoreCommand::ExecuteQuery { sql })
            .await;
        extract_query_result(events)
    }

    /// Fetch a specific page of the last executed query.
    #[uniffi::method(async_runtime = "tokio")]
    pub async fn fetch_page(&self, page: u32) -> Result<FfiQueryResult, SbqlFfiError> {
        let mut core = self.core.lock().await;
        let events = core
            .handle(sbql_core::CoreCommand::FetchPage {
                page: page as usize,
            })
            .await;
        extract_query_result(events)
    }

    // -------------------------------------------------------------------
    // Sort / Filter
    // -------------------------------------------------------------------

    /// Apply ORDER BY and re-execute.
    #[uniffi::method(async_runtime = "tokio")]
    pub async fn apply_order(
        &self,
        column: String,
        direction: FfiSortDirection,
    ) -> Result<FfiQueryResult, SbqlFfiError> {
        let mut core = self.core.lock().await;
        let events = core
            .handle(sbql_core::CoreCommand::ApplyOrder {
                column,
                direction: direction.into(),
            })
            .await;
        extract_query_result(events)
    }

    /// Remove ORDER BY and re-execute.
    #[uniffi::method(async_runtime = "tokio")]
    pub async fn clear_order(&self) -> Result<FfiQueryResult, SbqlFfiError> {
        let mut core = self.core.lock().await;
        let events = core.handle(sbql_core::CoreCommand::ClearOrder).await;
        extract_query_result(events)
    }

    /// Apply WHERE filter and re-execute.
    #[uniffi::method(async_runtime = "tokio")]
    pub async fn apply_filter(
        &self,
        query: String,
    ) -> Result<FfiQueryResult, SbqlFfiError> {
        let mut core = self.core.lock().await;
        let events = core
            .handle(sbql_core::CoreCommand::ApplyFilter { query })
            .await;
        extract_query_result(events)
    }

    /// Remove WHERE filter and re-execute.
    #[uniffi::method(async_runtime = "tokio")]
    pub async fn clear_filter(&self) -> Result<FfiQueryResult, SbqlFfiError> {
        let mut core = self.core.lock().await;
        let events = core.handle(sbql_core::CoreCommand::ClearFilter).await;
        extract_query_result(events)
    }

    /// Suggest distinct values for autocomplete.
    #[uniffi::method(async_runtime = "tokio")]
    pub async fn suggest_filter_values(
        &self,
        column: String,
        prefix: String,
        limit: u32,
        token: u64,
    ) -> Result<FfiFilterSuggestions, SbqlFfiError> {
        let mut core = self.core.lock().await;
        let events = core
            .handle(sbql_core::CoreCommand::SuggestFilterValues {
                column,
                prefix,
                limit: limit as usize,
                token,
            })
            .await;
        for ev in &events {
            if let sbql_core::CoreEvent::Error(msg) = ev {
                return Err(SbqlFfiError::core(msg));
            }
        }
        for ev in events {
            if let sbql_core::CoreEvent::FilterSuggestions { items, token } = ev {
                return Ok(FfiFilterSuggestions { items, token });
            }
        }
        Ok(FfiFilterSuggestions {
            items: vec![],
            token: 0,
        })
    }

    // -------------------------------------------------------------------
    // Mutations
    // -------------------------------------------------------------------

    /// Update a single cell.
    #[uniffi::method(async_runtime = "tokio")]
    pub async fn update_cell(
        &self,
        schema: String,
        table: String,
        pk_col: String,
        pk_val: String,
        target_col: String,
        new_val: String,
    ) -> Result<(), SbqlFfiError> {
        let mut core = self.core.lock().await;
        let events = core
            .handle(sbql_core::CoreCommand::UpdateCell {
                schema,
                table,
                pk_col,
                pk_val,
                target_col,
                new_val,
            })
            .await;
        check_for_error(events)
    }

    /// Delete a single row by primary key.
    #[uniffi::method(async_runtime = "tokio")]
    pub async fn delete_row(
        &self,
        schema: String,
        table: String,
        pk_col: String,
        pk_val: String,
    ) -> Result<(), SbqlFfiError> {
        let mut core = self.core.lock().await;
        let events = core
            .handle(sbql_core::CoreCommand::DeleteRow {
                schema,
                table,
                pk_col,
                pk_val,
            })
            .await;
        check_for_error(events)
    }
}

// ---------------------------------------------------------------------------
// Helper functions for extracting typed results from CoreEvents
// ---------------------------------------------------------------------------

fn parse_uuid(id: &str) -> Result<uuid::Uuid, SbqlFfiError> {
    uuid::Uuid::parse_str(id)
        .map_err(|e| SbqlFfiError::InvalidArgument { msg: format!("Invalid UUID '{id}': {e}") })
}

fn extract_connection_list(
    events: Vec<sbql_core::CoreEvent>,
) -> Result<Vec<FfiConnectionConfig>, SbqlFfiError> {
    for ev in &events {
        if let sbql_core::CoreEvent::Error(msg) = ev {
            return Err(SbqlFfiError::core(msg));
        }
    }
    for ev in events {
        if let sbql_core::CoreEvent::ConnectionList(list) = ev {
            return Ok(list.into_iter().map(Into::into).collect());
        }
    }
    Ok(vec![])
}

fn extract_query_result(
    events: Vec<sbql_core::CoreEvent>,
) -> Result<FfiQueryResult, SbqlFfiError> {
    for ev in &events {
        if let sbql_core::CoreEvent::Error(msg) = ev {
            return Err(SbqlFfiError::core(msg));
        }
    }
    for ev in events {
        if let sbql_core::CoreEvent::QueryResult(r) = ev {
            return Ok(r.into());
        }
    }
    // Return empty result for operations that don't produce a query result
    // (e.g. ClearOrder/ClearFilter when no active query)
    Ok(FfiQueryResult {
        columns: vec![],
        rows: vec![],
        page: 0,
        has_next_page: false,
    })
}

fn check_for_error(events: Vec<sbql_core::CoreEvent>) -> Result<(), SbqlFfiError> {
    for ev in events {
        if let sbql_core::CoreEvent::Error(msg) = ev {
            return Err(SbqlFfiError::core(msg));
        }
    }
    Ok(())
}
