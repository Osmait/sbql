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
pub mod query;
pub mod query_builder;
pub mod schema;

// Re-export the most commonly used types at the crate root.
pub use config::{load_connections, save_connections, ConnectionConfig, SslMode};
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
    password_cache: HashMap<Uuid, String>,
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
            // ----------------------------------------------------------------
            // Connection management
            // ----------------------------------------------------------------
            CoreCommand::SaveConnection { config, password } => {
                if let Some(ref pw) = password {
                    if let Err(e) = config.save_password(pw) {
                        tracing::warn!("Keyring save failed (will use in-memory cache): {e}");
                    }
                    // Cache the new password in memory for this session
                    self.password_cache.insert(config.id, pw.clone());
                } else {
                    // No new password supplied — make sure we still have the old
                    // one in the cache so a subsequent Connect works.
                    if !self.password_cache.contains_key(&config.id) {
                        // Try loading from keyring so the cache is warm
                        if let Ok(pw) = config.load_password() {
                            self.password_cache.insert(config.id, pw);
                        }
                    }
                }
                // Upsert into the in-memory list
                if let Some(pos) = self.connections.iter().position(|c| c.id == config.id) {
                    self.connections[pos] = config;
                } else {
                    self.connections.push(config);
                }
                if let Err(e) = save_connections(&self.connections) {
                    return vec![CoreEvent::Error(e.to_string())];
                }
                vec![CoreEvent::ConnectionList(self.connections.clone())]
            }

            CoreCommand::DeleteConnection(id) => {
                if let Some(pos) = self.connections.iter().position(|c| c.id == id) {
                    let cfg = self.connections.remove(pos);
                    let _ = cfg.delete_password();
                    self.manager.disconnect(id).await;
                }
                if let Err(e) = save_connections(&self.connections) {
                    return vec![CoreEvent::Error(e.to_string())];
                }
                vec![CoreEvent::ConnectionList(self.connections.clone())]
            }

            CoreCommand::Connect(id) => {
                let cfg = match self.connections.iter().find(|c| c.id == id) {
                    Some(c) => c.clone(),
                    None => {
                        return vec![CoreEvent::Error(format!(
                            "Connection {} not found",
                            id
                        ))]
                    }
                };
                // Prefer in-memory cache first to avoid repeated keychain prompts
                // during the same app session. Fall back to keyring if needed.
                let password = if let Some(pw) = self.password_cache.get(&id).cloned() {
                    Ok(pw)
                } else {
                    cfg.load_password().map(|pw| {
                        self.password_cache.insert(id, pw.clone());
                        pw
                    }).or_else(|_| {
                        Err(SbqlError::Keyring(
                            format!("No password found for '{}'. Try re-entering it (e to edit).", cfg.name)
                        ))
                    })
                };
                let password = match password {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!("Password lookup failed for '{}': {}", cfg.name, e);
                        return vec![CoreEvent::Error(e.to_string())];
                    }
                };
                match self.manager.connect_with_password(&cfg, &password).await {
                    Ok(()) => {
                        self.active_connection = Some(id);
                        tracing::info!("Connected: {}", cfg.name);
                        vec![CoreEvent::Connected(id)]
                    }
                    Err(e) => {
                        tracing::error!("Connect failed for '{}': {}", cfg.name, e);
                        vec![CoreEvent::Error(e.to_string())]
                    }
                }
            }

            CoreCommand::Disconnect(id) => {
                self.manager.disconnect(id).await;
                if self.active_connection == Some(id) {
                    self.active_connection = None;
                }
                vec![CoreEvent::Disconnected(id)]
            }

            // ----------------------------------------------------------------
            // Schema
            // ----------------------------------------------------------------
            CoreCommand::ListTables => {
                let pool = match self.active_pool() {
                    Ok(p) => p,
                    Err(e) => return vec![CoreEvent::Error(e.to_string())],
                };
                match schema::list_tables(&pool).await {
                    Ok(tables) => vec![CoreEvent::TableList(tables)],
                    Err(e) => vec![CoreEvent::Error(e.to_string())],
                }
            }

            // ----------------------------------------------------------------
            // Query execution
            // ----------------------------------------------------------------
            CoreCommand::ExecuteQuery { sql } => {
                self.base_sql = Some(sql.clone());
                self.effective_sql = Some(sql.clone());
                self.sort_state.clear();
                self.active_filter = None;
                self.execute_current_page(0).await
            }

            CoreCommand::FetchPage { page } => self.execute_current_page(page).await,

            // ----------------------------------------------------------------
            // Order / filter pushdown
            // ----------------------------------------------------------------
            CoreCommand::ApplyOrder { column, direction } => {
                let base = match &self.effective_sql {
                    Some(s) => s.clone(),
                    None => return vec![CoreEvent::Error("No active query".into())],
                };
                // Strip any existing ORDER BY before applying new one
                let without_order = query_builder::clear_order(&base)
                    .unwrap_or(base.clone());
                match query_builder::apply_order(&without_order, &column, direction) {
                    Ok(new_sql) => {
                        self.effective_sql = Some(new_sql);
                        self.sort_state.clear();
                        self.sort_state.insert(column, direction);
                        self.execute_current_page(0).await
                    }
                    Err(e) => vec![CoreEvent::Error(e.to_string())],
                }
            }

            CoreCommand::ClearOrder => {
                let effective = match &self.effective_sql {
                    Some(s) => s.clone(),
                    None => return vec![],
                };
                match query_builder::clear_order(&effective) {
                    Ok(new_sql) => {
                        self.effective_sql = Some(new_sql);
                        self.sort_state.clear();
                        self.execute_current_page(0).await
                    }
                    Err(e) => vec![CoreEvent::Error(e.to_string())],
                }
            }

            CoreCommand::ApplyFilter { query: filter } => {
                // Always apply the filter on top of the base SQL to avoid
                // stacking multiple WHERE clauses.
                let base = match &self.base_sql {
                    Some(s) => s.clone(),
                    None => return vec![CoreEvent::Error("No active query".into())],
                };
                let cols = if self.last_columns.is_empty() {
                    None
                } else {
                    Some(self.last_columns.as_slice())
                };
                match query_builder::apply_filter(&base, &filter, cols) {
                    Ok(filtered_sql) => {
                        // Re-apply existing sort on top of the filtered SQL
                        let final_sql = if let Some((col, &dir)) = self.sort_state.iter().next() {
                            query_builder::apply_order(&filtered_sql, col, dir)
                                .unwrap_or(filtered_sql)
                        } else {
                            filtered_sql
                        };
                        self.effective_sql = Some(final_sql);
                        self.active_filter = Some(filter);
                        self.execute_current_page(0).await
                    }
                    Err(e) => vec![CoreEvent::Error(e.to_string())],
                }
            }

            CoreCommand::ClearFilter => {
                self.active_filter = None;
                // Rebuild effective SQL from base + current sort
                let base = match &self.base_sql {
                    Some(s) => s.clone(),
                    None => return vec![],
                };
                let final_sql = if let Some((col, &dir)) = self.sort_state.iter().next() {
                    query_builder::apply_order(&base, col, dir).unwrap_or(base)
                } else {
                    base
                };
                self.effective_sql = Some(final_sql);
                self.execute_current_page(0).await
            }

            // ----------------------------------------------------------------
            // Primary key lookup
            // ----------------------------------------------------------------
            CoreCommand::GetPrimaryKeys { schema, table } => {
                let pool = match self.active_pool() {
                    Ok(p) => p,
                    Err(e) => return vec![CoreEvent::Error(e.to_string())],
                };
                match schema::get_primary_keys(&pool, &schema, &table).await {
                    Ok(columns) => vec![CoreEvent::PrimaryKeys { schema, table, columns }],
                    Err(e) => vec![CoreEvent::Error(e.to_string())],
                }
            }

            CoreCommand::LoadDiagram => {
                let pool = match self.active_pool() {
                    Ok(p) => p,
                    Err(e) => return vec![CoreEvent::Error(e.to_string())],
                };
                match schema::load_diagram(&pool).await {
                    Ok(data) => vec![CoreEvent::DiagramLoaded(data)],
                    Err(e) => vec![CoreEvent::Error(e.to_string())],
                }
            }

            // ----------------------------------------------------------------
            // Cell update
            // ----------------------------------------------------------------
            CoreCommand::UpdateCell {
                schema,
                table,
                pk_col,
                pk_val,
                target_col,
                new_val,
            } => {
                let pool = match self.active_pool() {
                    Ok(p) => p,
                    Err(e) => return vec![CoreEvent::Error(e.to_string())],
                };
                match schema::execute_cell_update(
                    &pool,
                    &schema,
                    &table,
                    &pk_col,
                    &pk_val,
                    &target_col,
                    &new_val,
                )
                .await
                {
                    Ok(()) => {
                        vec![CoreEvent::CellUpdated]
                    }
                    Err(e) => vec![CoreEvent::Error(e.to_string())],
                }
            }

            CoreCommand::DeleteRow {
                schema,
                table,
                pk_col,
                pk_val,
            } => {
                let pool = match self.active_pool() {
                    Ok(p) => p,
                    Err(e) => return vec![CoreEvent::Error(e.to_string())],
                };
                match schema::execute_row_delete(&pool, &schema, &table, &pk_col, &pk_val).await {
                    Ok(()) => vec![CoreEvent::RowDeleted],
                    Err(e) => vec![CoreEvent::Error(e.to_string())],
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn active_pool(&self) -> Result<sqlx::PgPool> {
        let id = self
            .active_connection
            .ok_or(SbqlError::NoActiveConnection)?;
        // ConnectionManager::get is async; we use block_in_place since this is
        // called from within an already-running tokio multi-thread runtime.
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.manager.get(id))
        })
    }

    async fn execute_current_page(&mut self, page: usize) -> Vec<CoreEvent> {
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
                self.last_columns = result.columns.clone();
                self.last_page = result.page;
                vec![CoreEvent::QueryResult(result)]
            }
            Err(e) => vec![CoreEvent::Error(e.to_string())],
        }
    }
}
