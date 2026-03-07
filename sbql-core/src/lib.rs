//! `sbql-core` — headless SQL editor library.
//!
//! This crate is completely UI-agnostic. It exposes a single [`Core`] struct
//! that the TUI (or any other frontend) drives via [`CoreCommand`] values and
//! receives responses from via [`CoreEvent`] values.
//!
//! ## Threading model
//!
//! `Core` is `Clone + Send + Sync`. The TUI spawns a dedicated Tokio task
//! that owns a `Core` instance and processes commands sequentially, sending
//! events back over an `mpsc` channel.

pub mod config;
pub mod connection;
pub mod error;
mod handlers;
pub mod query;
pub mod query_builder;
pub mod schema;

// Re-export the most commonly used types at the crate root.
pub use config::{
    load_connections, load_connections_from, save_connections, save_connections_to,
    ConnectionConfig, SslMode,
};
pub use error::{Result, SbqlError};
pub use query::{QueryResult, PAGE_SIZE};
pub use query_builder::SortDirection;
pub use schema::{ColumnInfo, DiagramData, ForeignKey, TableEntry, TableSchema};

use std::collections::HashMap;
use uuid::Uuid;

use connection::ConnectionManager;

// ---------------------------------------------------------------------------
// Public command / event enums
// ---------------------------------------------------------------------------

/// Commands sent from the UI layer → Core.
#[derive(Debug, Clone)]
pub enum CoreCommand {
    /// Persist a new or updated connection config (password stored in keyring).
    /// Pass `Some(password)` to set/replace the password, or `None` to keep
    /// the existing password unchanged (useful when editing without re-entering).
    SaveConnection {
        config: ConnectionConfig,
        password: Option<String>,
    },
    /// Remove a connection config from disk and keyring.
    DeleteConnection(Uuid),
    /// Open a connection pool for the given connection id.
    Connect(Uuid),
    /// Close the pool for a connection.
    Disconnect(Uuid),
    /// List all tables in the currently active connection.
    ListTables,
    /// Execute a raw SQL string, page 0.
    ExecuteQuery { sql: String },
    /// Fetch a specific page of the last executed query.
    FetchPage { page: usize },
    /// Re-execute with an ORDER BY injected via AST manipulation.
    ApplyOrder {
        column: String,
        direction: SortDirection,
    },
    /// Remove the current ORDER BY and re-execute.
    ClearOrder,
    /// Re-execute with a WHERE filter injected via AST manipulation.
    ApplyFilter { query: String },
    /// Remove the current WHERE filter and re-execute.
    ClearFilter,
    /// Suggest distinct values for `column` matching `prefix%`.
    SuggestFilterValues {
        column: String,
        prefix: String,
        limit: usize,
        token: u64,
    },
    /// Fetch primary key columns for a given table.
    GetPrimaryKeys { schema: String, table: String },
    /// Load all table schemas and FK relationships for the diagram view.
    LoadDiagram,
    /// Update a single cell in the database.
    UpdateCell {
        schema: String,
        table: String,
        pk_col: String,
        pk_val: String,
        target_col: String,
        new_val: String,
    },
    /// Delete a single row identified by its primary key.
    DeleteRow {
        schema: String,
        table: String,
        pk_col: String,
        pk_val: String,
    },
}

/// Events sent from Core → UI.
#[derive(Debug, Clone)]
pub enum CoreEvent {
    /// The full list of saved connections (sent on startup and after mutations).
    ConnectionList(Vec<ConnectionConfig>),
    /// A connection pool was opened successfully.
    Connected(Uuid),
    /// A connection pool was closed.
    Disconnected(Uuid),
    /// Table list for the current connection.
    TableList(Vec<TableEntry>),
    /// Query result page.
    QueryResult(QueryResult),
    /// A cell UPDATE completed successfully.
    CellUpdated,
    /// A row DELETE completed successfully.
    RowDeleted,
    /// Primary key columns for a table.
    PrimaryKeys {
        schema: String,
        table: String,
        columns: Vec<String>,
    },
    /// Full diagram data (table schemas + FK relationships).
    DiagramLoaded(DiagramData),
    /// Filter value suggestions response.
    FilterSuggestions { items: Vec<String>, token: u64 },
    /// A long-running operation has started (show a spinner).
    Loading,
    /// An error occurred.
    Error(String),
}

// ---------------------------------------------------------------------------
// Core state
// ---------------------------------------------------------------------------

/// The central headless state machine.
///
/// Holds live connection pools and the current query context.
/// Designed to be driven from a single async task.
#[derive(Clone, Default)]
pub struct Core {
    /// All saved connection configs (loaded from disk).
    pub connections: Vec<ConnectionConfig>,
    /// Live connection pools.
    pub manager: ConnectionManager,
    /// The currently active connection id.
    pub active_connection: Option<Uuid>,
    /// The "base" SQL query entered by the user (without ORDER BY / WHERE mods).
    pub base_sql: Option<String>,
    /// The "effective" SQL that includes any active ORDER BY / WHERE modifiers.
    pub effective_sql: Option<String>,
    /// Column names from the last query result (needed for global filter).
    pub last_columns: Vec<String>,
    /// The page number of the most recently returned query result.
    pub last_page: usize,
    /// Active sort state: column name → direction.
    pub sort_state: HashMap<String, SortDirection>,
    /// Active filter string (raw, as the user typed it).
    pub active_filter: Option<String>,
    /// In-memory password cache so reconnects work even if keyring lookup fails.
    pub(crate) password_cache: HashMap<Uuid, String>,
}

impl Core {
    /// Create a new Core and load connections from disk.
    pub fn new() -> Self {
        let connections = load_connections().unwrap_or_default();
        Self {
            connections,
            ..Default::default()
        }
    }

    /// Process a single [`CoreCommand`] and return zero or more [`CoreEvent`]s.
    pub async fn handle(&mut self, cmd: CoreCommand) -> Vec<CoreEvent> {
        match cmd {
            CoreCommand::SaveConnection { config, password } => {
                handlers::connection::save(self, config, password).await
            }
            CoreCommand::DeleteConnection(id) => handlers::connection::delete(self, id).await,
            CoreCommand::Connect(id) => handlers::connection::connect(self, id).await,
            CoreCommand::Disconnect(id) => handlers::connection::disconnect(self, id).await,
            CoreCommand::ListTables => handlers::schema::list_tables(self).await,
            CoreCommand::ExecuteQuery { sql } => handlers::query::execute(self, sql).await,
            CoreCommand::FetchPage { page } => handlers::query::fetch_page(self, page).await,
            CoreCommand::ApplyOrder { column, direction } => {
                handlers::order_filter::apply_order(self, column, direction).await
            }
            CoreCommand::ClearOrder => handlers::order_filter::clear_order(self).await,
            CoreCommand::ApplyFilter { query } => {
                handlers::order_filter::apply_filter(self, query).await
            }
            CoreCommand::ClearFilter => handlers::order_filter::clear_filter(self).await,
            CoreCommand::SuggestFilterValues {
                column,
                prefix,
                limit,
                token,
            } => {
                handlers::order_filter::suggest_filter_values(self, column, prefix, limit, token)
                    .await
            }
            CoreCommand::GetPrimaryKeys { schema, table } => {
                handlers::schema::get_primary_keys(self, schema, table).await
            }
            CoreCommand::LoadDiagram => handlers::schema::load_diagram(self).await,
            CoreCommand::UpdateCell {
                schema,
                table,
                pk_col,
                pk_val,
                target_col,
                new_val,
            } => {
                handlers::mutation::update_cell(
                    self, schema, table, pk_col, pk_val, target_col, new_val,
                )
                .await
            }
            CoreCommand::DeleteRow {
                schema,
                table,
                pk_col,
                pk_val,
            } => handlers::mutation::delete_row(self, schema, table, pk_col, pk_val).await,
        }
    }

    // -----------------------------------------------------------------------
    // Helpers used by handler modules
    // -----------------------------------------------------------------------

    pub(crate) fn active_pool(&self) -> Result<sqlx::PgPool> {
        let id = self
            .active_connection
            .ok_or(SbqlError::NoActiveConnection)?;
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.manager.get(id))
        })
    }

    pub(crate) async fn execute_current_page(&mut self, page: usize) -> Vec<CoreEvent> {
        let sql = match &self.effective_sql {
            Some(s) => s.clone(),
            None => return vec![CoreEvent::Error("No active query".into())],
        };
        let pool = match self.active_pool() {
            Ok(p) => p,
            Err(e) => return vec![CoreEvent::Error(e.to_string())],
        };
        match query::execute_page(&pool, &sql, page).await {
            Ok(result) => {
                if !result.columns.is_empty() {
                    self.last_columns = result.columns.clone();
                }
                self.last_page = result.page;
                vec![CoreEvent::QueryResult(result)]
            }
            Err(e) => vec![CoreEvent::Error(e.to_string())],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_initialization() {
        let core = Core::new();
        // Just verify it doesn't crash on default initialization
        assert!(core.active_connection.is_none());
        assert!(core.base_sql.is_none());
        assert!(core.effective_sql.is_none());
    }

    #[tokio::test]
    async fn test_core_handle_unknown_connection() {
        let mut core = Core::new();
        // Ensure no connections exist
        core.connections.clear();

        // Handling Connect with a nonexistent UUID should yield an error event
        let random_id = Uuid::new_v4();
        let events = core.handle(CoreCommand::Connect(random_id)).await;

        assert_eq!(events.len(), 1);
        match &events[0] {
            CoreEvent::Error(msg) => assert!(msg.contains("not found")),
            _ => panic!("Expected error event"),
        }
    }

    #[tokio::test]
    async fn test_core_handle_disconnect_without_active() {
        let mut core = Core::new();
        let id = Uuid::new_v4();

        let events = core.handle(CoreCommand::Disconnect(id)).await;
        assert_eq!(events.len(), 1);
        match &events[0] {
            CoreEvent::Disconnected(disconnected_id) => assert_eq!(*disconnected_id, id),
            _ => panic!("Expected disconnected event"),
        }
    }

    #[tokio::test]
    async fn test_core_handle_query_without_connection() {
        let mut core = Core::new();
        // Sending a query when disconnected should fail
        let events = core
            .handle(CoreCommand::ExecuteQuery {
                sql: "SELECT 1".into(),
            })
            .await;
        assert_eq!(events.len(), 1);
        match &events[0] {
            CoreEvent::Error(msg) => assert!(msg.contains("No active connection")),
            _ => panic!("Expected error event"),
        }
    }

    #[tokio::test]
    async fn test_core_handle_schema_without_connection() {
        let mut core = Core::new();
        let events = core.handle(CoreCommand::ListTables).await;
        assert_eq!(events.len(), 1);
        match &events[0] {
            CoreEvent::Error(msg) => assert!(msg.contains("No active connection")),
            _ => panic!("Expected error event"),
        }
    }

    // -----------------------------------------------------------------------
    // Query handler state tests (no DB needed)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_execute_query_sets_sql_state() {
        let mut core = Core::default();
        // Execute will fail (no pool) but should set base_sql/effective_sql first
        core.active_connection = None;
        let _events = core
            .handle(CoreCommand::ExecuteQuery {
                sql: "SELECT 1".into(),
            })
            .await;
        assert_eq!(core.base_sql, Some("SELECT 1".into()));
        assert_eq!(core.effective_sql, Some("SELECT 1".into()));
        assert!(core.sort_state.is_empty());
        assert!(core.active_filter.is_none());
    }

    // -----------------------------------------------------------------------
    // Order/filter handler state tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_apply_order_without_active_query() {
        let mut core = Core::default();
        core.effective_sql = None;
        let events = core
            .handle(CoreCommand::ApplyOrder {
                column: "id".into(),
                direction: SortDirection::Ascending,
            })
            .await;
        assert_eq!(events.len(), 1);
        match &events[0] {
            CoreEvent::Error(msg) => assert!(msg.contains("No active query")),
            _ => panic!("Expected error event"),
        }
    }

    #[tokio::test]
    async fn test_apply_filter_without_base_sql() {
        let mut core = Core::default();
        core.base_sql = None;
        let events = core
            .handle(CoreCommand::ApplyFilter {
                query: "name:Alice".into(),
            })
            .await;
        assert_eq!(events.len(), 1);
        match &events[0] {
            CoreEvent::Error(msg) => assert!(msg.contains("No active query")),
            _ => panic!("Expected error event"),
        }
    }

    #[tokio::test]
    async fn test_clear_filter_without_base_sql() {
        let mut core = Core::default();
        core.base_sql = None;
        let events = core.handle(CoreCommand::ClearFilter).await;
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn test_clear_order_without_effective_sql() {
        let mut core = Core::default();
        core.effective_sql = None;
        let events = core.handle(CoreCommand::ClearOrder).await;
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_page_without_effective_sql() {
        let mut core = Core::default();
        core.effective_sql = None;
        let events = core.handle(CoreCommand::FetchPage { page: 0 }).await;
        assert_eq!(events.len(), 1);
        match &events[0] {
            CoreEvent::Error(msg) => assert!(msg.contains("No active query")),
            _ => panic!("Expected error event"),
        }
    }

    #[tokio::test]
    async fn test_suggest_filter_values_no_connection() {
        let mut core = Core::default();
        core.base_sql = Some("SELECT 1".into());
        // No active connection → Error
        let events = core
            .handle(CoreCommand::SuggestFilterValues {
                column: "name".into(),
                prefix: "A".into(),
                limit: 10,
                token: 1,
            })
            .await;
        assert_eq!(events.len(), 1);
        match &events[0] {
            CoreEvent::Error(msg) => assert!(msg.contains("No active connection")),
            _ => panic!("Expected Error event"),
        }
    }

    #[tokio::test]
    async fn test_disconnect_clears_active() {
        let mut core = Core::default();
        let id = Uuid::new_v4();
        core.active_connection = Some(id);
        let events = core.handle(CoreCommand::Disconnect(id)).await;
        assert!(core.active_connection.is_none());
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], CoreEvent::Disconnected(d) if *d == id));
    }
}
