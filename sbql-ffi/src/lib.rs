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
    Mysql,
    Sqlite,
    Redis,
    DynamoDb,
    MongoDb,
    SqlServer,
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

#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiExportFormat {
    Csv,
    Json,
    SqlInsert,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiImportFormat {
    Csv,
    Json,
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
    pub ssh_enabled: bool,
    pub ssh_host: String,
    pub ssh_port: u16,
    pub ssh_user: String,
    pub ssh_auth_method: String,
    pub ssh_key_path: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiQueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub page: u32,
    pub has_next_page: bool,
    pub total_count: Option<u64>,
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
        Self::Core { msg: msg.into() }
    }
}

// ---------------------------------------------------------------------------
// SbqlEngine — the FFI Object
// ---------------------------------------------------------------------------

#[derive(uniffi::Object)]
pub struct SbqlEngine {
    core: Arc<Mutex<sbql_core::Core>>,

    /// Held, never read — and it must stay that way.
    ///
    /// This engine builds its own multi-threaded runtime in [`Self::new`].
    /// Dropping the last `Arc` to a `Runtime` shuts its worker threads down,
    /// so this field is what keeps the runtime alive for as long as Swift
    /// holds the engine. Delete it as "unused" and every `async` method here
    /// loses the executor underneath it.
    ///
    /// `#[allow(dead_code)]` because that is exactly what it looks like to the
    /// compiler, which cannot see a `Drop` impl as a use.
    #[allow(dead_code)]
    runtime: Arc<tokio::runtime::Runtime>,
}

impl Default for SbqlEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[uniffi::export]
impl SbqlEngine {
    /// Create a new engine, loading saved connections from disk.
    #[uniffi::constructor]
    pub fn new() -> Self {
        // The one panic we keep. A `#[uniffi::constructor]` cannot return a
        // `Result` here, and an engine with no runtime cannot answer a single
        // call — there is nothing to degrade to. In practice this only fails
        // if the OS refuses to spawn threads, at which point the process is
        // over anyway.
        #[allow(clippy::expect_used)]
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
        ssh_password: Option<String>,
    ) -> Result<Vec<FfiConnectionConfig>, SbqlFfiError> {
        let core_config: sbql_core::ConnectionConfig = config.try_into()?;
        // The SSH password used to be written to the keyring right here, before
        // the command was dispatched — and therefore before `validate()` ran
        // inside the save handler. A config the handler then rejected left the
        // secret behind under an id that never reached `connections.toml`. It
        // now rides in the command and is written beside the database password,
        // on the far side of validation. The Swift-facing signature is
        // unchanged; only where the value is written moved.
        let mut core = self.core.lock().await;
        let events = core
            .handle(sbql_core::CoreCommand::SaveConnection {
                config: core_config,
                password,
                ssh_password,
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
        if let Some(failure) = first_failure(&events) {
            return Err(failure);
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
        if let Some(failure) = first_failure(&events) {
            return Err(failure);
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
        if let Some(failure) = first_failure(&events) {
            return Err(failure);
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
    pub async fn execute_query(&self, sql: String) -> Result<FfiQueryResult, SbqlFfiError> {
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

    /// Total row count for the current query, or `nil` when there is none to
    /// be had (no active query, an unsupported backend, an expensive query
    /// shape, or a count that took too long).
    ///
    /// Separate from `execute_query` on purpose: the count used to ride along
    /// with page 0 and held the rows back by up to three seconds. Call this
    /// after the page has been shown.
    #[uniffi::method(async_runtime = "tokio")]
    pub async fn fetch_total_count(&self) -> Result<Option<u64>, SbqlFfiError> {
        let mut core = self.core.lock().await;
        let events = core.handle(sbql_core::CoreCommand::FetchTotalCount).await;
        extract_total_count(events)
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
    pub async fn apply_filter(&self, query: String) -> Result<FfiQueryResult, SbqlFfiError> {
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
        if let Some(failure) = first_failure(&events) {
            return Err(failure);
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
    // Export
    // -------------------------------------------------------------------

    /// Stream all rows of the current query to a file.
    #[uniffi::method(async_runtime = "tokio")]
    pub async fn export_all(
        &self,
        path: String,
        format: FfiExportFormat,
        table_name: String,
    ) -> Result<u64, SbqlFfiError> {
        let core = self.core.lock().await;
        let fmt: sbql_core::ExportFormat = match format {
            FfiExportFormat::Csv => sbql_core::ExportFormat::Csv,
            FfiExportFormat::Json => sbql_core::ExportFormat::Json,
            FfiExportFormat::SqlInsert => sbql_core::ExportFormat::SqlInsert,
        };
        core.export_all(&path, fmt, &table_name)
            .await
            .map_err(|e| SbqlFfiError::core(e.to_string()))
    }

    // -------------------------------------------------------------------
    // Import
    // -------------------------------------------------------------------

    /// Import a CSV or JSON file into a database table.
    #[uniffi::method(async_runtime = "tokio")]
    pub async fn import_file(
        &self,
        path: String,
        format: FfiImportFormat,
        schema: String,
        table_name: String,
    ) -> Result<u64, SbqlFfiError> {
        let core = self.core.lock().await;
        let fmt = match format {
            FfiImportFormat::Csv => sbql_core::ImportFormat::Csv,
            FfiImportFormat::Json => sbql_core::ImportFormat::Json,
        };
        core.import_file(&path, fmt, &schema, &table_name)
            .await
            .map_err(|e| SbqlFfiError::core(e.to_string()))
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
        // The FFI surface stays single-column for now; the core takes the full
        // composite key, so this wraps the one pair it has.
        let events = core
            .handle(sbql_core::CoreCommand::UpdateCell {
                schema,
                table,
                pk: vec![(pk_col, pk_val)],
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
                pk: vec![(pk_col, pk_val)],
            })
            .await;
        check_for_error(events)
    }

    /// Format SQL by parsing into AST and re-serializing with consistent style.
    #[uniffi::method]
    pub fn format_sql(&self, sql: String) -> String {
        sbql_core::format_sql(&sql)
    }
}

// ---------------------------------------------------------------------------
// Helper functions for extracting typed results from CoreEvents
// ---------------------------------------------------------------------------

fn parse_uuid(id: &str) -> Result<uuid::Uuid, SbqlFfiError> {
    uuid::Uuid::parse_str(id).map_err(|e| SbqlFfiError::InvalidArgument {
        msg: format!("Invalid UUID '{id}': {e}"),
    })
}

/// The first genuine failure in `events`, if there is one.
///
/// Warnings are deliberately skipped. `CoreEvent::Error` also carries the
/// "it worked, but…" case — saving a connection whose password the keyring
/// refused, say — and throwing that across the FFI tells the caller their save
/// failed when the connection is sitting on disk. Before `CoreError` had a
/// severity there was no way to tell the two apart here.
fn first_failure(events: &[sbql_core::CoreEvent]) -> Option<SbqlFfiError> {
    events.iter().find_map(|ev| match ev {
        sbql_core::CoreEvent::Error(e) if !e.is_warning() => {
            // `Display` includes the cause chain, which is the part worth
            // showing: the summary alone is often just "Database error".
            Some(SbqlFfiError::core(e.to_string()))
        }
        _ => None,
    })
}

fn extract_connection_list(
    events: Vec<sbql_core::CoreEvent>,
) -> Result<Vec<FfiConnectionConfig>, SbqlFfiError> {
    if let Some(failure) = first_failure(&events) {
        return Err(failure);
    }
    for ev in events {
        if let sbql_core::CoreEvent::ConnectionList(list) = ev {
            return Ok(list.into_iter().map(Into::into).collect());
        }
    }
    Ok(vec![])
}

fn extract_query_result(events: Vec<sbql_core::CoreEvent>) -> Result<FfiQueryResult, SbqlFfiError> {
    if let Some(failure) = first_failure(&events) {
        return Err(failure);
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
        total_count: None,
    })
}

fn extract_total_count(events: Vec<sbql_core::CoreEvent>) -> Result<Option<u64>, SbqlFfiError> {
    if let Some(failure) = first_failure(&events) {
        return Err(failure);
    }
    for ev in events {
        if let sbql_core::CoreEvent::TotalCount(count) = ev {
            return Ok(count);
        }
    }
    // No count event is the same answer as a count that could not be taken.
    Ok(None)
}

fn check_for_error(events: Vec<sbql_core::CoreEvent>) -> Result<(), SbqlFfiError> {
    if let Some(failure) = first_failure(&events) {
        return Err(failure);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    /// Keep the test suite away from the developer's machine.
    ///
    /// `SbqlEngine::new()` builds a real `Core`, and `save_connection` really
    /// persists — so without this every run rewrote the developer's own
    /// `~/.config/sbql/connections.toml`, and a suite that saves nothing but
    /// its own fixtures left it holding `connections = []`. That is exactly
    /// how a real set of saved connections gets destroyed by running the
    /// tests. `sbql-core` has guarded this since its first test; the FFI
    /// suite never did.
    ///
    /// Passwords are kept out of the OS credential store for the same reason:
    /// on a desktop with a locked keyring every run would otherwise pop an
    /// unlock prompt and leave test credentials behind.
    ///
    /// The temp dir is created once per test process and leaked, so it
    /// outlives every test that reads it back.
    fn isolate_from_the_machine() {
        static SCRATCH: OnceLock<tempfile::TempDir> = OnceLock::new();
        let dir = SCRATCH.get_or_init(|| {
            #[allow(clippy::expect_used)]
            tempfile::tempdir().expect("create temp config dir")
        });
        std::env::set_var(sbql_core::CONFIG_DIR_ENV, dir.path());
        std::env::set_var(sbql_core::NO_KEYRING_ENV, "1");
    }

    // --- parse_uuid tests ---

    #[test]
    fn parse_uuid_empty_string() {
        assert!(parse_uuid("").is_err());
    }

    #[test]
    fn parse_uuid_invalid() {
        assert!(parse_uuid("not-a-uuid").is_err());
    }

    #[test]
    fn parse_uuid_valid() {
        let result = parse_uuid("550e8400-e29b-41d4-a716-446655440000");
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().to_string(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
    }

    // --- extract_connection_list tests ---

    #[test]
    fn extract_connection_list_with_error() {
        let events = vec![sbql_core::CoreEvent::Error(sbql_core::CoreError::new(
            sbql_core::ErrorKind::Other,
            "boom",
        ))];
        let result = extract_connection_list(events);
        assert!(result.is_err());
        match result.unwrap_err() {
            SbqlFfiError::Core { msg } => assert_eq!(msg, "boom"),
            _ => panic!("Expected Core error"),
        }
    }

    #[test]
    fn extract_connection_list_with_list() {
        let config =
            sbql_core::ConnectionConfig::new_postgres("test", "localhost", 5432, "user", "db");
        let events = vec![sbql_core::CoreEvent::ConnectionList(vec![config])];
        let result = extract_connection_list(events).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "test");
    }

    #[test]
    fn extract_connection_list_empty_events() {
        let events = vec![];
        let result = extract_connection_list(events).unwrap();
        assert!(result.is_empty());
    }

    // --- extract_query_result tests ---

    #[test]
    fn extract_query_result_with_error() {
        let events = vec![sbql_core::CoreEvent::Error(sbql_core::CoreError::new(
            sbql_core::ErrorKind::Query,
            "query failed",
        ))];
        let result = extract_query_result(events);
        assert!(result.is_err());
    }

    #[test]
    fn extract_query_result_with_result() {
        let qr = sbql_core::QueryResult {
            columns: vec!["id".into()],
            rows: vec![vec!["1".into()]],
            page: 0,
            has_next_page: false,
            total_count: None,
        };
        let events = vec![sbql_core::CoreEvent::QueryResult(qr)];
        let result = extract_query_result(events).unwrap();
        assert_eq!(result.columns, vec!["id"]);
        assert_eq!(result.page, 0);
    }

    #[test]
    fn extract_query_result_empty_events() {
        let events = vec![];
        let result = extract_query_result(events).unwrap();
        assert!(result.columns.is_empty());
    }

    // --- extract_total_count tests ---

    #[test]
    fn extract_total_count_with_count() {
        let events = vec![sbql_core::CoreEvent::TotalCount(Some(42))];
        assert_eq!(extract_total_count(events).unwrap(), Some(42));
    }

    /// A count that could not be taken is `nil`, not a thrown error — Swift
    /// asked for a number it can live without.
    #[test]
    fn extract_total_count_without_count() {
        let events = vec![sbql_core::CoreEvent::TotalCount(None)];
        assert_eq!(extract_total_count(events).unwrap(), None);
    }

    #[test]
    fn extract_total_count_with_error() {
        let events = vec![sbql_core::CoreEvent::Error(sbql_core::CoreError::new(
            sbql_core::ErrorKind::Query,
            "boom",
        ))];
        assert!(extract_total_count(events).is_err());
    }

    /// `SortChanged` is one of the events every command can now carry along.
    /// The extractors have to walk past it, not stop at it.
    #[test]
    fn extract_query_result_ignores_sort_changed() {
        let qr = sbql_core::QueryResult {
            columns: vec!["id".into()],
            rows: vec![vec!["1".into()]],
            page: 0,
            has_next_page: false,
            total_count: None,
        };
        let events = vec![
            sbql_core::CoreEvent::SortChanged(Some((
                "id".into(),
                sbql_core::SortDirection::Ascending,
            ))),
            sbql_core::CoreEvent::QueryResult(qr),
        ];
        let result = extract_query_result(events).unwrap();
        assert_eq!(result.columns, vec!["id"]);
    }

    // --- check_for_error tests ---

    #[test]
    fn check_for_error_no_errors() {
        let events = vec![
            sbql_core::CoreEvent::CellUpdated,
            sbql_core::CoreEvent::RowDeleted,
        ];
        assert!(check_for_error(events).is_ok());
    }

    #[test]
    fn check_for_error_with_error() {
        let events = vec![
            sbql_core::CoreEvent::CellUpdated,
            sbql_core::CoreEvent::Error(sbql_core::CoreError::new(
                sbql_core::ErrorKind::Other,
                "failed",
            )),
        ];
        assert!(check_for_error(events).is_err());
    }

    // --- SbqlEngine smoke tests ---

    #[test]
    fn engine_new_does_not_panic() {
        isolate_from_the_machine();
        let _engine = SbqlEngine::new();
    }

    #[test]
    fn engine_get_connections_initially_empty_or_loaded() {
        isolate_from_the_machine();
        let engine = SbqlEngine::new();
        // Should not panic; returns whatever is on disk (may be empty or not)
        let _conns = engine.get_connections();
    }

    // SbqlEngine creates its own tokio runtime, so these tests use
    // #[tokio::test] with spawn_blocking to drop the engine off the async context.

    #[tokio::test]
    async fn engine_connect_nonexistent_id() {
        isolate_from_the_machine();
        let engine = SbqlEngine::new();
        let result = engine
            .connect("550e8400-e29b-41d4-a716-446655440000".into())
            .await;
        assert!(result.is_err());
        tokio::task::spawn_blocking(move || drop(engine))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn engine_save_connection_invalid_uuid() {
        isolate_from_the_machine();
        let engine = SbqlEngine::new();
        let config = FfiConnectionConfig {
            id: "invalid-uuid".into(),
            name: "test".into(),
            backend: FfiDbBackend::Sqlite,
            host: "".into(),
            port: 0,
            user: "".into(),
            database: "".into(),
            ssl_mode: FfiSslMode::Prefer,
            file_path: None,
            ssh_enabled: false,
            ssh_host: String::new(),
            ssh_port: 22,
            ssh_user: String::new(),
            ssh_auth_method: String::new(),
            ssh_key_path: None,
        };
        let result = engine.save_connection(config, None, None).await;
        assert!(result.is_err());
        tokio::task::spawn_blocking(move || drop(engine))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn engine_full_lifecycle_sqlite() {
        isolate_from_the_machine();
        let engine = SbqlEngine::new();
        let id = uuid::Uuid::new_v4().to_string();
        let config = FfiConnectionConfig {
            id: id.clone(),
            name: "lifecycle_test".into(),
            backend: FfiDbBackend::Sqlite,
            host: "".into(),
            port: 0,
            user: "".into(),
            database: "".into(),
            ssl_mode: FfiSslMode::Prefer,
            file_path: Some(":memory:".into()),
            ssh_enabled: false,
            ssh_host: String::new(),
            ssh_port: 22,
            ssh_user: String::new(),
            ssh_auth_method: String::new(),
            ssh_key_path: None,
        };

        // Save
        let list = engine.save_connection(config, None, None).await.unwrap();
        assert!(list.iter().any(|c| c.id == id));

        // Connect
        engine.connect(id.clone()).await.unwrap();

        // List tables (SQLite in-memory has none by default)
        let tables = engine.list_tables().await.unwrap();
        assert!(tables.is_empty());

        // Execute a query
        let result = engine
            .execute_query("SELECT 1 AS val".into())
            .await
            .unwrap();
        assert_eq!(result.columns, vec!["val"]);
        assert_eq!(result.rows.len(), 1);

        // Disconnect
        engine.disconnect(id.clone()).await.unwrap();

        // Clean up: delete the connection. Best-effort on purpose — the
        // assertions above are what this test is about, and the scratch config
        // dir goes away with the process either way. `drop` rather than
        // `let _ =` so the discard is deliberate rather than a `Result` that
        // slipped through unread.
        drop(engine.delete_connection(id).await);
        tokio::task::spawn_blocking(move || drop(engine))
            .await
            .unwrap();
    }
}
