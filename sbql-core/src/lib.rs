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
pub mod connection_spec;
pub mod discovery;
pub mod error;
mod handlers;
pub mod import;
pub mod pool;
pub mod query;
pub mod query_builder;
pub mod schema;
pub mod sql_util;
pub mod tunnel;

// Re-export the most commonly used types at the crate root.
pub use config::{
    config_path, keyring_enabled, load_connections, load_connections_from, save_connections,
    save_connections_to, ConnectionConfig, SslMode, CONFIG_DIR_ENV, NO_KEYRING_ENV,
};
pub use connection_spec::{
    BackendSpec, ConnectionDraft, ConnectionField, FieldSpec, ValidationError,
};
pub use discovery::{DiscoveredConnection, DiscoverySource};
pub use error::{CoreError, ErrorKind, Result, SbqlError, Severity};
pub use import::ImportFormat;
pub use pool::{DbBackend, DbPool};
pub use query::{ExportFormat, QueryResult, PAGE_SIZE};
pub use query_builder::{format_sql, SortDirection};
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
    /// Ask Docker which databases are running and offer them as connections.
    ///
    /// `dir` is the directory the client was opened in; the compose project
    /// rooted there is listed first. Discovered connections are session-only —
    /// nothing is written to disk or the keyring until the user asks for it
    /// with [`CoreCommand::SaveDiscovered`].
    DiscoverConnections { dir: std::path::PathBuf },
    /// Promote a discovered connection to a saved one, password included.
    SaveDiscovered(Uuid),
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
    /// Count the rows the current query would return, out of band.
    ///
    /// Deliberately its own command: counting used to happen inside page 0,
    /// which meant every query's first page waited on a `COUNT(*)` that most
    /// clients never display.
    FetchTotalCount,
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
    ///
    /// `pk` carries every `(column, value)` component of the row's primary
    /// key. Sending only the first component of a composite key once turned
    /// "update this row" into "update every row sharing that component".
    UpdateCell {
        schema: String,
        table: String,
        pk: Vec<(String, String)>,
        target_col: String,
        new_val: String,
    },
    /// Delete a single row identified by its full primary key.
    DeleteRow {
        schema: String,
        table: String,
        pk: Vec<(String, String)>,
    },
}

impl CoreCommand {
    /// Whether the UI should show a progress indicator while this runs.
    ///
    /// The match is exhaustive on purpose. This used to be a blacklist in the
    /// TUI worker, so every new command silently defaulted to blanking the
    /// results pane until someone remembered to add it — a background lookup
    /// would flicker the whole UI. Now a new command cannot compile without
    /// saying which kind it is.
    pub fn shows_progress(&self) -> bool {
        match self {
            // Background lookups. Fast, and not what the user is waiting on,
            // so they must not blank the UI.
            CoreCommand::GetPrimaryKeys { .. }
            | CoreCommand::Disconnect(_)
            | CoreCommand::LoadDiagram
            | CoreCommand::SuggestFilterValues { .. }
            | CoreCommand::FetchTotalCount
            // Runs at startup before the user has asked for anything; a
            // spinner here would look like the app is stuck on launch.
            | CoreCommand::DiscoverConnections { .. } => false,

            // Work the user asked for and is waiting on.
            CoreCommand::SaveConnection { .. }
            | CoreCommand::SaveDiscovered(_)
            | CoreCommand::DeleteConnection(_)
            | CoreCommand::Connect(_)
            | CoreCommand::ListTables
            | CoreCommand::ExecuteQuery { .. }
            | CoreCommand::FetchPage { .. }
            | CoreCommand::ApplyOrder { .. }
            | CoreCommand::ClearOrder
            | CoreCommand::ApplyFilter { .. }
            | CoreCommand::ClearFilter
            | CoreCommand::UpdateCell { .. }
            | CoreCommand::DeleteRow { .. } => true,
        }
    }
}

/// Events sent from Core → UI.
#[derive(Debug, Clone)]
pub enum CoreEvent {
    /// The full list of saved connections (sent on startup and after mutations).
    ConnectionList(Vec<ConnectionConfig>),
    /// Databases found running in Docker, most relevant first.
    ///
    /// Carries no passwords: this event is debug-logged on its way to the UI,
    /// and the credentials scraped out of a container have no business in a
    /// log file. Core keeps them in its session cache instead.
    DiscoveredConnections(Vec<DiscoveredConnection>),
    /// A connection pool was opened successfully.
    Connected(Uuid),
    /// A connection pool was closed.
    Disconnected(Uuid),
    /// Table list for the current connection.
    TableList(Vec<TableEntry>),
    /// Query result page.
    QueryResult(QueryResult),
    /// Total row count for the current query, or `None` when there isn't one
    /// to be had — see [`query::total_count`]. Only ever sent in reply to
    /// [`CoreCommand::FetchTotalCount`].
    TotalCount(Option<u64>),
    /// The sort core is now applying, or `None` for "no sort".
    ///
    /// Core owns this. Clients used to keep their own copy and update it
    /// optimistically, which drifted the moment core dropped the sort without
    /// being asked — disconnecting still left the TUI drawing a sort arrow for
    /// an ORDER BY that was no longer in the query.
    SortChanged(Option<(String, SortDirection)>),
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
    /// Something went wrong — or went through with a caveat, see
    /// [`CoreError::severity`].
    Error(CoreError),
}

impl CoreEvent {
    /// Report a failure, classifying it and keeping its cause chain.
    ///
    /// The single place `SbqlError` becomes client-facing, so no handler has to
    /// remember to do more than `to_string()`.
    pub fn error(e: impl Into<CoreError>) -> Self {
        CoreEvent::Error(e.into())
    }
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
    /// The sort currently baked into `effective_sql`, if any.
    ///
    /// One sort at a time — this was a `HashMap` that handlers cleared before
    /// every insert and read back with `.iter().next()`, so it could only ever
    /// hold the one entry anyway.
    pub sort_state: Option<(String, SortDirection)>,
    /// Active filter string (raw, as the user typed it).
    pub active_filter: Option<String>,
    /// Databases found running in Docker this session.
    ///
    /// Deliberately not part of `connections`: that list is what gets written
    /// to `connections.toml`, and scraping a container's password is not
    /// consent to persist it. These live for the session only, until the user
    /// promotes one with [`CoreCommand::SaveDiscovered`].
    pub discovered: Vec<DiscoveredConnection>,
    /// In-memory password cache so reconnects work even if keyring lookup fails.
    pub(crate) password_cache: HashMap<Uuid, String>,
    /// Why the saved connections could not be read, if they could not.
    ///
    /// Held rather than returned because [`Core::new`] cannot fail — clients
    /// collect it from [`Core::startup_events`].
    pub(crate) load_error: Option<CoreError>,
}

impl Core {
    /// Create a new Core and load connections from disk.
    ///
    /// A config file that cannot be parsed is *not* quietly treated as "no
    /// connections saved". That is what used to happen, and the result is
    /// indistinguishable from a fresh install: the user is invited to add the
    /// connections they already have, while the file that still holds them
    /// sits there unread.
    pub fn new() -> Self {
        let (connections, load_error) = match load_connections() {
            Ok(list) => (list, None),
            Err(e) => {
                tracing::error!("Could not read saved connections: {e}");
                (
                    Vec::new(),
                    Some(
                        CoreError::new(
                            ErrorKind::Config,
                            "Saved connections could not be read — none are available",
                        )
                        .with_detail(e.to_string()),
                    ),
                )
            }
        };

        Self {
            connections,
            load_error,
            ..Default::default()
        }
    }

    /// Forget everything tied to the previously active query/session: the base
    /// and effective SQL, the last result's columns and page, and any active
    /// sort or filter. Called when a connection closes so the next session does
    /// not inherit the last one's state.
    ///
    /// Returns the [`CoreEvent::SortChanged`] the client still has to hear, if
    /// a sort was dropped. It comes back rather than being emitted here so the
    /// `#[must_use]` catches a caller that forgets: a client left holding the
    /// old sort keeps drawing an indicator for an ORDER BY that is gone, which
    /// is exactly the bug this reset used to cause.
    #[must_use = "the dropped sort has to reach the client"]
    pub(crate) fn reset_query_state(&mut self) -> Option<CoreEvent> {
        self.base_sql = None;
        self.effective_sql = None;
        self.last_columns.clear();
        self.last_page = 0;
        self.active_filter = None;
        self.set_sort(None)
    }

    /// Record the sort now baked into `effective_sql`.
    ///
    /// Returns the event announcing it, or `None` when the sort did not
    /// actually move — core is the single owner of the applied sort, and
    /// clients cache only what this reports, so an event that says nothing new
    /// is pure noise.
    #[must_use = "a changed sort has to reach the client"]
    pub(crate) fn set_sort(&mut self, sort: Option<(String, SortDirection)>) -> Option<CoreEvent> {
        if self.sort_state == sort {
            return None;
        }
        self.sort_state = sort;
        Some(CoreEvent::SortChanged(self.sort_state.clone()))
    }

    /// What a client should be told before it sends its first command.
    ///
    /// The connection list, plus anything that went wrong producing it. Lives
    /// here rather than in each frontend so the TUI and the macOS app cannot
    /// disagree about what startup looks like.
    pub fn startup_events(&self) -> Vec<CoreEvent> {
        let mut events = vec![CoreEvent::ConnectionList(self.connections.clone())];
        if let Some(err) = &self.load_error {
            events.push(CoreEvent::Error(err.clone()));
        }
        events
    }

    /// Process a single [`CoreCommand`] and return zero or more [`CoreEvent`]s.
    pub async fn handle(&mut self, cmd: CoreCommand) -> Vec<CoreEvent> {
        match cmd {
            CoreCommand::SaveConnection { config, password } => {
                handlers::connection::save(self, config, password).await
            }
            CoreCommand::DeleteConnection(id) => handlers::connection::delete(self, id).await,
            CoreCommand::DiscoverConnections { dir } => {
                handlers::connection::discover(self, dir).await
            }
            CoreCommand::SaveDiscovered(id) => {
                handlers::connection::save_discovered(self, id).await
            }
            CoreCommand::Connect(id) => handlers::connection::connect(self, id).await,
            CoreCommand::Disconnect(id) => handlers::connection::disconnect(self, id).await,
            CoreCommand::ListTables => handlers::schema::list_tables(self).await,
            CoreCommand::ExecuteQuery { sql } => handlers::query::execute(self, sql).await,
            CoreCommand::FetchPage { page } => handlers::query::fetch_page(self, page).await,
            CoreCommand::FetchTotalCount => handlers::query::fetch_total_count(self).await,
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
                pk,
                target_col,
                new_val,
            } => {
                handlers::mutation::update_cell(self, schema, table, pk, target_col, new_val).await
            }
            CoreCommand::DeleteRow { schema, table, pk } => {
                handlers::mutation::delete_row(self, schema, table, pk).await
            }
        }
    }

    // -----------------------------------------------------------------------
    // Helpers used by handler modules
    // -----------------------------------------------------------------------

    /// Import a CSV or JSON file into a database table.
    pub async fn import_file(
        &self,
        path: &str,
        format: ImportFormat,
        schema: &str,
        table: &str,
    ) -> Result<u64> {
        let pool = self.active_pool().await?;
        import::import_file(&pool, path, format, schema, table).await
    }

    /// Stream all rows of the current effective SQL to a file.
    pub async fn export_all(
        &self,
        path: &str,
        format: ExportFormat,
        table_name: &str,
    ) -> Result<u64> {
        let sql = self
            .effective_sql
            .as_ref()
            .ok_or(SbqlError::Config("No active query".into()))?;
        let pool = self.active_pool().await?;
        query::export_all(&pool, sql, path, format, table_name).await
    }

    pub(crate) async fn active_pool(&self) -> Result<DbPool> {
        let id = self
            .active_connection
            .ok_or(SbqlError::NoActiveConnection)?;
        self.manager.get(id).await
    }

    pub(crate) fn active_backend(&self) -> Result<DbBackend> {
        let id = self
            .active_connection
            .ok_or(SbqlError::NoActiveConnection)?;
        let cfg = self
            .connections
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| SbqlError::ConnectionNotFound(id.to_string()))?;
        Ok(cfg.backend)
    }

    pub(crate) async fn execute_current_page(&mut self, page: usize) -> Vec<CoreEvent> {
        let sql = match &self.effective_sql {
            Some(s) => s.clone(),
            None => {
                return vec![CoreEvent::Error(CoreError::new(
                    ErrorKind::Query,
                    "No active query",
                ))]
            }
        };
        let pool = match self.active_pool().await {
            Ok(p) => p,
            Err(e) => return vec![CoreEvent::error(&e)],
        };
        match query::execute_page(&pool, &sql, page).await {
            Ok(result) => {
                if !result.columns.is_empty() {
                    self.last_columns = result.columns.clone();
                }
                self.last_page = result.page;
                vec![CoreEvent::QueryResult(result)]
            }
            Err(e) => vec![CoreEvent::error(&e)],
        }
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
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

    /// Normally there is nothing to report beyond the list itself.
    #[test]
    fn startup_says_only_what_there_is_to_say() {
        let core = Core::default();
        let events = core.startup_events();

        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], CoreEvent::ConnectionList(_)));
    }

    /// An unreadable config file used to look exactly like an empty one. The
    /// client has to be able to tell the difference, or it invites the user to
    /// re-add connections that are still sitting on disk.
    #[test]
    fn startup_reports_connections_it_could_not_read() {
        let core = Core {
            load_error: Some(
                CoreError::new(ErrorKind::Config, "Saved connections could not be read")
                    .with_detail("expected `=` at line 3"),
            ),
            ..Default::default()
        };

        let events = core.startup_events();

        assert!(
            matches!(&events[0], CoreEvent::ConnectionList(list) if list.is_empty()),
            "the empty list still comes first: {events:?}"
        );
        match &events[1] {
            CoreEvent::Error(e) => {
                assert_eq!(e.kind, ErrorKind::Config);
                assert!(!e.is_warning(), "losing the connection list is a failure");
                assert_eq!(e.detail.as_deref(), Some("expected `=` at line 3"));
            }
            other => panic!("expected the load failure, got {other:?}"),
        }
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
            CoreEvent::Error(e) => assert!(e.message.contains("not found")),
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
            CoreEvent::Error(e) => assert!(e.message.contains("No active connection")),
            _ => panic!("Expected error event"),
        }
    }

    #[tokio::test]
    async fn test_core_handle_schema_without_connection() {
        let mut core = Core::new();
        let events = core.handle(CoreCommand::ListTables).await;
        assert_eq!(events.len(), 1);
        match &events[0] {
            CoreEvent::Error(e) => assert!(e.message.contains("No active connection")),
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
        assert!(core.sort_state.is_none());
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
            CoreEvent::Error(e) => assert!(e.message.contains("No active query")),
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
            CoreEvent::Error(e) => assert!(e.message.contains("No active query")),
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
            CoreEvent::Error(e) => assert!(e.message.contains("No active query")),
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
            CoreEvent::Error(e) => assert!(e.message.contains("No active connection")),
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

    /// Closing the active connection drops its sort along with the rest of the
    /// query state — and the client has to be told, or it goes on drawing a
    /// sort indicator for an ORDER BY that no query is applying any more.
    #[tokio::test]
    async fn disconnecting_reports_the_sort_it_dropped() {
        let mut core = Core::default();
        let id = Uuid::new_v4();
        core.active_connection = Some(id);
        core.sort_state = Some(("name".into(), SortDirection::Ascending));

        let events = core.handle(CoreCommand::Disconnect(id)).await;

        assert!(core.sort_state.is_none());
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CoreEvent::SortChanged(None))),
            "{events:?}"
        );
    }

    /// Disconnecting some *other* connection leaves the active session — and
    /// its sort — alone, so there is nothing to report about it.
    #[tokio::test]
    async fn disconnecting_an_inactive_connection_says_nothing_about_sort() {
        let mut core = Core::default();
        core.active_connection = Some(Uuid::new_v4());
        core.sort_state = Some(("name".into(), SortDirection::Ascending));

        let events = core.handle(CoreCommand::Disconnect(Uuid::new_v4())).await;

        assert!(core.sort_state.is_some());
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, CoreEvent::SortChanged(_))),
            "{events:?}"
        );
    }
}
