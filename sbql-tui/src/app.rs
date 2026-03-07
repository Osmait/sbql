use std::collections::HashMap;
use std::time::Instant;

use ratatui::layout::Rect;
use sbql_core::{
    ConnectionConfig, CoreEvent, DbBackend, DiagramData, QueryResult, SortDirection, SslMode,
    TableEntry,
};
use tui_textarea::TextArea;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Focus model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedPanel {
    Connections,
    Tables,
    Editor,
    Results,
}

// ---------------------------------------------------------------------------
// Vim-style editor mode (applies to the SQL editor panel)
// ---------------------------------------------------------------------------

/// Two-mode model for the SQL editor panel.
///
/// - `Normal`: cursor moves with hjkl; `i` enters Insert.
/// - `Insert`: full tui-textarea editing; `Esc` returns to Normal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Normal,
    Insert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavMode {
    Global,
    Panel,
}

// ---------------------------------------------------------------------------
// Connection form state (add/edit connection dialog)
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct ConnectionForm {
    pub visible: bool,
    pub editing_id: Option<Uuid>, // None = new connection
    pub field_index: usize,       // which field is active
    pub backend: DbBackend,
    pub name: String,
    pub host: String,
    pub port: String,
    pub user: String,
    pub database: String,
    pub password: String,
    pub ssl_mode: SslMode,
    pub file_path: String,
    pub error: Option<String>,
}

impl Default for ConnectionForm {
    fn default() -> Self {
        Self {
            visible: false,
            editing_id: None,
            field_index: 0,
            backend: DbBackend::Postgres,
            name: String::new(),
            host: String::new(),
            port: String::new(),
            user: String::new(),
            database: String::new(),
            password: String::new(),
            ssl_mode: SslMode::Prefer,
            file_path: String::new(),
            error: None,
        }
    }
}

impl ConnectionForm {
    pub fn open_new() -> Self {
        Self {
            visible: true,
            port: "5432".into(),
            host: "localhost".into(),
            ssl_mode: SslMode::Prefer,
            ..Default::default()
        }
    }

    pub fn open_edit(cfg: &ConnectionConfig) -> Self {
        Self {
            visible: true,
            editing_id: Some(cfg.id),
            backend: cfg.backend,
            name: cfg.name.clone(),
            host: cfg.host.clone(),
            port: cfg.port.to_string(),
            user: cfg.user.clone(),
            database: cfg.database.clone(),
            password: String::new(), // always re-enter
            ssl_mode: cfg.ssl_mode.clone(),
            file_path: cfg.file_path.clone().unwrap_or_default(),
            field_index: 0,
            error: None,
        }
    }

    /// Returns the label for each field index, depending on backend.
    pub fn field_label(&self, idx: usize) -> &'static str {
        match self.backend {
            DbBackend::Postgres => match idx {
                0 => "Backend",
                1 => "Name",
                2 => "Host",
                3 => "Port",
                4 => "User",
                5 => "Database",
                6 => "Password",
                7 => "SSL Mode",
                _ => "",
            },
            DbBackend::Sqlite => match idx {
                0 => "Backend",
                1 => "Name",
                2 => "File Path",
                _ => "",
            },
        }
    }

    /// Number of fields depends on the selected backend.
    pub fn field_count(&self) -> usize {
        match self.backend {
            DbBackend::Postgres => 8, // backend, name, host, port, user, database, password, ssl_mode
            DbBackend::Sqlite => 3,   // backend, name, file_path
        }
    }

    /// Toggle backend between Postgres and SQLite, resetting field_index.
    pub fn cycle_backend(&mut self) {
        self.backend = match self.backend {
            DbBackend::Postgres => DbBackend::Sqlite,
            DbBackend::Sqlite => DbBackend::Postgres,
        };
        self.field_index = 0;
    }

    /// Cycle through SSL mode options (for the SSL Mode field).
    pub fn cycle_ssl_mode(&mut self) {
        self.ssl_mode = match self.ssl_mode {
            SslMode::Prefer => SslMode::Require,
            SslMode::Require => SslMode::VerifyFull,
            SslMode::VerifyFull => SslMode::VerifyCa,
            SslMode::VerifyCa => SslMode::Disable,
            SslMode::Disable => SslMode::Prefer,
        };
    }

    pub fn active_value_mut(&mut self) -> Option<&mut String> {
        match self.backend {
            DbBackend::Postgres => match self.field_index {
                0 => None, // Backend is cycled
                1 => Some(&mut self.name),
                2 => Some(&mut self.host),
                3 => Some(&mut self.port),
                4 => Some(&mut self.user),
                5 => Some(&mut self.database),
                6 => Some(&mut self.password),
                7 => None, // SSL mode is cycled
                _ => None,
            },
            DbBackend::Sqlite => match self.field_index {
                0 => None, // Backend is cycled
                1 => Some(&mut self.name),
                2 => Some(&mut self.file_path),
                _ => None,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Cell edit overlay state
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct CellEditState {
    pub row_idx: usize,
    pub col_idx: usize,
    pub col_name: String,
    pub original: String,
    /// Table info needed to generate the UPDATE statement.
    pub schema: String,
    pub table: String,
    pub pk_col: String,
    pub pk_val: String,
    pub textarea: TextArea<'static>,
}

impl CellEditState {
    pub fn new(
        row_idx: usize,
        col_idx: usize,
        col_name: String,
        original: String,
        schema: String,
        table: String,
        pk_col: String,
        pk_val: String,
    ) -> Self {
        let mut ta = TextArea::default();
        ta.insert_str(&original);
        Self {
            row_idx,
            col_idx,
            col_name,
            original,
            schema,
            table,
            pk_col,
            pk_val,
            textarea: ta,
        }
    }

    pub fn current_value(&self) -> String {
        self.textarea.lines().join("\n")
    }
}

// ---------------------------------------------------------------------------
// Filter bar state
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct FilterBar {
    pub visible: bool,
    pub textarea: TextArea<'static>,
    pub suggestions: Vec<String>,
    pub selected_suggestion: usize,
    pub show_suggestions: bool,
    pub suggestion_token: u64,
    pub loading_suggestions: bool,
    pub pending_live_apply_at: Option<Instant>,
    pub last_applied_query: Option<String>,
}

impl Default for FilterBar {
    fn default() -> Self {
        Self {
            visible: false,
            textarea: TextArea::default(),
            suggestions: Vec::new(),
            selected_suggestion: 0,
            show_suggestions: false,
            suggestion_token: 0,
            loading_suggestions: false,
            pending_live_apply_at: None,
            last_applied_query: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Pending (staged) edit model
// ---------------------------------------------------------------------------

/// A cell edit that has been staged locally but not yet committed to the DB.
#[derive(Debug, Clone)]
pub struct PendingEdit {
    pub new_val: String,
    /// The original value before editing (kept for potential diff display).
    #[allow(dead_code)]
    pub original: String,
    pub schema: String,
    pub table: String,
    pub pk_col: String,
    pub pk_val: String,
    pub col_name: String,
}

/// A row marked for deletion, with its PK already resolved.
#[derive(Debug, Clone)]
pub struct PendingDelete {
    pub schema: String,
    pub table: String,
    pub pk_col: String,
    pub pk_val: String,
}

// ---------------------------------------------------------------------------
// Diagram mode state
// ---------------------------------------------------------------------------

/// State for the full-screen database diagram view.
pub struct DiagramState {
    pub data: DiagramData,
    /// Index of the selected table in the left sidebar list.
    pub selected_table: usize,
    /// Horizontal scroll offset for the right diagram canvas (in characters).
    pub scroll_x: u16,
    /// Vertical scroll offset for the right diagram canvas (in rows).
    pub scroll_y: u16,
    /// When true, only show FKs connected to the selected table.
    pub focus_mode: bool,
    /// Glyph rendering mode for diagram connectors/boxes.
    pub glyph_mode: DiagramGlyphMode,
    /// Cached canvas lines from the last render.
    pub cached_canvas: Option<Vec<ratatui::text::Line<'static>>>,
    /// When true, the cached canvas must be rebuilt.
    pub canvas_dirty: bool,
    /// Stored table positions (global table index → (x, y)) from the last layout.
    pub table_positions: HashMap<usize, (usize, usize)>,
    /// Last known viewport width (set during draw).
    pub last_viewport_w: u16,
    /// Last known viewport height (set during draw).
    pub last_viewport_h: u16,
    /// Search mode active in diagram sidebar.
    pub search_active: bool,
    /// Current search query string.
    pub search_query: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagramGlyphMode {
    Ascii,
    Unicode,
}

impl DiagramState {
    pub fn new(data: DiagramData) -> Self {
        Self {
            data,
            selected_table: 0,
            scroll_x: 0,
            scroll_y: 0,
            focus_mode: false,
            glyph_mode: DiagramGlyphMode::Ascii,
            cached_canvas: None,
            canvas_dirty: true,
            table_positions: HashMap::new(),
            last_viewport_w: 0,
            last_viewport_h: 0,
            search_active: false,
            search_query: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Composable sub-states
// ---------------------------------------------------------------------------

pub struct ConnectionState {
    pub connections: Vec<ConnectionConfig>,
    pub selected: usize,
    pub active_id: Option<Uuid>,
    pub active_backend: DbBackend,
    pub form: ConnectionForm,
    pub pending_delete: Option<(Uuid, String)>,
}

pub struct TableBrowserState {
    pub tables: Vec<TableEntry>,
    pub selected: usize,
}

pub struct EditorState {
    pub textarea: TextArea<'static>,
    pub mode: EditorMode,
}

impl EditorState {
    /// Get the current SQL text from the editor.
    pub fn sql(&self) -> String {
        self.textarea.lines().join("\n")
    }
}

pub struct ResultsState {
    pub data: QueryResult,
    pub scroll: usize,
    pub col_scroll: usize,
    pub selected_row: usize,
    pub selected_col: usize,
    pub current_page: usize,
    pub is_loading: bool,
    pub sort_state: HashMap<String, SortDirection>,
    /// Height of the results viewport in rows (updated each draw cycle).
    pub viewport_height: usize,
    /// Number of visible columns in the results viewport (updated each draw cycle).
    pub viewport_cols: usize,
    /// When true, column widths need to be recomputed (data changed).
    pub col_widths_dirty: bool,
    /// Cached column widths from the last compute_col_widths call.
    pub cached_col_widths: Vec<u16>,
}

impl ResultsState {
    pub fn move_row_down_with_page_hint(&mut self) -> bool {
        let len = self.data.rows.len();
        if len == 0 {
            return false;
        }
        let max = len - 1;
        if self.selected_row < max {
            self.selected_row += 1;
            self.clamp_scroll();
            false
        } else if self.data.has_next_page {
            true
        } else {
            false
        }
    }

    pub fn move_row_up(&mut self) {
        if self.selected_row > 0 {
            self.selected_row -= 1;
            self.clamp_scroll();
        }
    }

    /// Adjust `scroll` so `selected_row` stays within the viewport.
    fn clamp_scroll(&mut self) {
        let vh = self.viewport_height.max(1);
        if self.selected_row < self.scroll {
            self.scroll = self.selected_row;
        } else if self.selected_row >= self.scroll + vh {
            self.scroll = self.selected_row + 1 - vh;
        }
    }

    pub fn move_col_right(&mut self) {
        let max = self.data.columns.len().saturating_sub(1);
        if self.selected_col < max {
            self.selected_col += 1;
            self.clamp_col_scroll();
        }
    }

    pub fn move_col_left(&mut self) {
        if self.selected_col > 0 {
            self.selected_col -= 1;
            self.clamp_col_scroll();
        }
    }

    /// Jump to the first row (vim `gg`).
    pub fn move_row_first(&mut self) {
        self.selected_row = 0;
        self.clamp_scroll();
    }

    /// Jump to the last row of the current page (vim `G`).
    pub fn move_row_last(&mut self) {
        let len = self.data.rows.len();
        if len > 0 {
            self.selected_row = len - 1;
            self.clamp_scroll();
        }
    }

    /// Move down by half the viewport height (vim `Ctrl+d`).
    pub fn move_row_half_page_down(&mut self) -> bool {
        let half = (self.viewport_height / 2).max(1);
        let len = self.data.rows.len();
        if len == 0 {
            return false;
        }
        let max = len - 1;
        if self.selected_row + half <= max {
            self.selected_row += half;
            self.clamp_scroll();
            false
        } else if self.data.has_next_page {
            self.selected_row = max;
            self.clamp_scroll();
            true
        } else {
            self.selected_row = max;
            self.clamp_scroll();
            false
        }
    }

    /// Move up by half the viewport height (vim `Ctrl+u`).
    pub fn move_row_half_page_up(&mut self) {
        let half = (self.viewport_height / 2).max(1);
        self.selected_row = self.selected_row.saturating_sub(half);
        self.clamp_scroll();
    }

    /// Jump to the first column (vim `0` / `^`).
    pub fn move_col_first(&mut self) {
        self.selected_col = 0;
        self.clamp_col_scroll();
    }

    /// Jump to the last column (vim `$`).
    pub fn move_col_last(&mut self) {
        let max = self.data.columns.len().saturating_sub(1);
        self.selected_col = max;
        self.clamp_col_scroll();
    }

    /// Adjust `col_scroll` so `selected_col` stays within the visible
    /// column window. `viewport_cols` is set by the draw cycle.
    fn clamp_col_scroll(&mut self) {
        let vc = self.viewport_cols.max(1);
        if self.selected_col < self.col_scroll {
            self.col_scroll = self.selected_col;
        } else if self.selected_col >= self.col_scroll + vc {
            self.col_scroll = self.selected_col + 1 - vc;
        }
    }

    /// Return the column name under the current cursor.
    pub fn selected_column_name(&self) -> Option<&str> {
        self.data
            .columns
            .get(self.selected_col)
            .map(String::as_str)
    }

    /// Toggle the sort direction for a column. Cycles None → Asc → Desc → None.
    pub fn toggle_sort(&mut self, col: &str) -> (String, Option<SortDirection>) {
        let next = match self.sort_state.get(col) {
            None => Some(SortDirection::Ascending),
            Some(SortDirection::Ascending) => Some(SortDirection::Descending),
            Some(SortDirection::Descending) => None,
        };
        match next {
            Some(dir) => {
                self.sort_state.clear();
                self.sort_state.insert(col.to_owned(), dir);
                (col.to_owned(), Some(dir))
            }
            None => {
                self.sort_state.remove(col);
                (col.to_owned(), None)
            }
        }
    }
}

pub struct MutationState {
    pub cell_edit: Option<CellEditState>,
    pub pending_cell_edit: Option<(usize, usize)>,
    pub pending_edits: HashMap<(usize, usize), PendingEdit>,
    pub pending_deletes: HashMap<usize, PendingDelete>,
    pub pending_delete_row: Option<usize>,
    pub pending_d: bool,
}

impl MutationState {
    /// Discard all staged (uncommitted) edits and deletes.
    pub fn discard_pending(&mut self) {
        self.pending_edits.clear();
        self.pending_deletes.clear();
        self.pending_delete_row = None;
        self.pending_d = false;
    }
}

pub struct VimState {
    pub nav_mode: NavMode,
    pub pending_leader: bool,
    pub pending_g: bool,
}

pub struct LayoutCache {
    pub last_areas: Option<LastAreas>,
    pub last_col_widths: Vec<u16>,
    pub spinner_frame: usize,
    pub sidebar_hidden: bool,
    /// When false, skip the terminal.draw() call to avoid redundant repaints.
    pub needs_redraw: bool,
}

// ---------------------------------------------------------------------------
// Main application state
// ---------------------------------------------------------------------------

pub struct AppState {
    // ---- panels ----
    pub focused: FocusedPanel,

    // ---- composable sub-states ----
    pub conn: ConnectionState,
    pub tables: TableBrowserState,
    pub editor: EditorState,
    pub results: ResultsState,
    pub mutation: MutationState,
    pub vim: VimState,
    pub layout: LayoutCache,

    // ---- filter ----
    pub filter: FilterBar,
    /// The currently active filter string (set locally when filter is applied).
    pub active_filter: Option<String>,

    // ---- diagram mode ----
    /// When Some, the diagram full-screen overlay is active.
    pub diagram: Option<DiagramState>,

    // ---- status / error ----
    pub status_msg: Option<String>,
    pub error_msg: Option<String>,

    // ---- quit ----
    pub should_quit: bool,
}

/// Snapshot of the rects from the last draw cycle.
#[derive(Debug, Clone, Copy)]
pub struct LastAreas {
    pub conn_list: Rect,
    pub table_list: Rect,
    pub editor: Rect,
    pub results: Rect,
}

impl AppState {
    pub fn new(connections: Vec<ConnectionConfig>) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_placeholder_text("-- Write SQL here. Press Ctrl+S or F5 to run.");

        Self {
            focused: FocusedPanel::Connections,

            conn: ConnectionState {
                connections,
                selected: 0,
                active_id: None,
                active_backend: DbBackend::Postgres,
                form: ConnectionForm::default(),
                pending_delete: None,
            },
            tables: TableBrowserState {
                tables: Vec::new(),
                selected: 0,
            },
            editor: EditorState {
                textarea,
                mode: EditorMode::Normal,
            },
            results: ResultsState {
                data: QueryResult::default(),
                scroll: 0,
                col_scroll: 0,
                selected_row: 0,
                selected_col: 0,
                current_page: 0,
                is_loading: false,
                sort_state: HashMap::new(),
                viewport_height: 20,
                viewport_cols: 5,
                col_widths_dirty: true,
                cached_col_widths: Vec::new(),
            },
            mutation: MutationState {
                cell_edit: None,
                pending_cell_edit: None,
                pending_edits: HashMap::new(),
                pending_deletes: HashMap::new(),
                pending_delete_row: None,
                pending_d: false,
            },
            vim: VimState {
                nav_mode: NavMode::Global,
                pending_leader: false,
                pending_g: false,
            },
            layout: LayoutCache {
                last_areas: None,
                last_col_widths: Vec::new(),
                spinner_frame: 0,
                sidebar_hidden: false,
                needs_redraw: true,
            },

            filter: FilterBar::default(),
            active_filter: None,
            diagram: None,
            status_msg: None,
            error_msg: None,
            should_quit: false,
        }
    }

    // -----------------------------------------------------------------------
    // Event application
    // -----------------------------------------------------------------------

    /// Apply an incoming [`CoreEvent`] to the application state.
    pub fn apply_core_event(&mut self, event: CoreEvent) {
        self.layout.needs_redraw = true;
        match event {
            CoreEvent::ConnectionList(conns) => {
                self.results.is_loading = false;
                self.conn.connections = conns;
                if self.conn.selected >= self.conn.connections.len() {
                    self.conn.selected = self.conn.connections.len().saturating_sub(1);
                }
            }
            CoreEvent::Connected(id) => {
                self.results.is_loading = false;
                self.conn.active_id = Some(id);
                if let Some(cfg) = self.conn.connections.iter().find(|c| c.id == id) {
                    self.conn.active_backend = cfg.backend;
                }
                let name = self
                    .conn
                    .connections
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| id.to_string());
                self.status_msg = Some(format!("Connected to {name}"));
                self.error_msg = None;
            }
            CoreEvent::Disconnected(id) => {
                self.results.is_loading = false;
                if self.conn.active_id == Some(id) {
                    self.conn.active_id = None;
                }
                self.tables.tables.clear();
                self.status_msg = Some("Disconnected".into());
            }
            CoreEvent::TableList(tables) => {
                self.results.is_loading = false;
                self.tables.tables = tables;
                self.tables.selected = 0;
            }
            CoreEvent::QueryResult(mut result) => {
                self.results.is_loading = false;
                self.results.current_page = result.page;
                tracing::info!(
                    "QueryResult: page={} rows={} cols={} has_next={}",
                    result.page,
                    result.rows.len(),
                    result.columns.len(),
                    result.has_next_page,
                );
                if result.page == 0 {
                    self.results.scroll = 0;
                    self.results.col_scroll = 0;
                    self.results.selected_row = 0;
                    self.results.selected_col = 0;
                }
                // Always discard staged changes when a new result set arrives
                self.mutation.discard_pending();

                // Preserve previous columns when current page has no rows.
                if result.columns.is_empty() && !self.results.data.columns.is_empty() {
                    result.columns = self.results.data.columns.clone();
                }

                self.results.data = result;
                self.results.col_widths_dirty = true;
                self.error_msg = None;
                self.status_msg = None;
            }
            CoreEvent::CellUpdated => {
                self.results.is_loading = false;
                self.mutation.cell_edit = None;
            }
            CoreEvent::RowDeleted => {
                self.results.is_loading = false;
            }
            CoreEvent::PrimaryKeys {
                schema,
                table,
                columns,
            } => {
                self.results.is_loading = false;
                let pk_col = columns.into_iter().next().unwrap_or_default();

                // Resolve a pending delete if one is waiting for this PK info.
                if let Some(row_idx) = self.mutation.pending_delete_row.take() {
                    let pk_val = self
                        .results
                        .data
                        .columns
                        .iter()
                        .position(|c| c.to_lowercase() == pk_col.to_lowercase())
                        .and_then(|pk_ci| {
                            self.results
                                .data
                                .rows
                                .get(row_idx)
                                .and_then(|r| r.get(pk_ci))
                                .cloned()
                        })
                        .unwrap_or_default();

                    if pk_col.is_empty() || pk_val.is_empty() {
                        self.error_msg =
                            Some("Cannot mark for delete: primary key not found.".into());
                    } else {
                        // Toggle: if already marked, unmark
                        if self.mutation.pending_deletes.contains_key(&row_idx) {
                            self.mutation.pending_deletes.remove(&row_idx);
                        } else {
                            self.mutation.pending_deletes.insert(
                                row_idx,
                                PendingDelete {
                                    schema,
                                    table,
                                    pk_col,
                                    pk_val,
                                },
                            );
                        }
                    }
                    return;
                }

                // Otherwise handle a pending cell edit waiting for this PK info.
                if let Some((row_idx, col_idx)) = self.mutation.pending_cell_edit.take() {
                    let col_name = match self.results.data.columns.get(col_idx) {
                        Some(c) => c.clone(),
                        None => return,
                    };
                    let original = self
                        .results
                        .data
                        .rows
                        .get(row_idx)
                        .and_then(|r| r.get(col_idx))
                        .cloned()
                        .unwrap_or_default();
                    // Find PK value in result row
                    let pk_val = self
                        .results
                        .data
                        .columns
                        .iter()
                        .position(|c| c.to_lowercase() == pk_col.to_lowercase())
                        .and_then(|pk_ci| {
                            self.results
                                .data
                                .rows
                                .get(row_idx)
                                .and_then(|r| r.get(pk_ci))
                                .cloned()
                        })
                        .unwrap_or_default();

                    if pk_col.is_empty() || pk_val.is_empty() {
                        self.error_msg =
                            Some("Cannot edit: primary key value not found in result set.".into());
                        return;
                    }

                    self.mutation.cell_edit = Some(CellEditState::new(
                        row_idx, col_idx, col_name, original, schema, table, pk_col, pk_val,
                    ));
                }
            }
            CoreEvent::Loading => {
                self.results.is_loading = true;
                self.error_msg = None;
            }
            CoreEvent::DiagramLoaded(data) => {
                self.results.is_loading = false;
                self.diagram = Some(DiagramState::new(data));
            }
            CoreEvent::FilterSuggestions { items, token } => {
                self.results.is_loading = false;
                if self.filter.visible && token == self.filter.suggestion_token {
                    if !items.is_empty() {
                        let mut merged = self.filter.suggestions.clone();
                        for item in items {
                            if !merged.iter().any(|x| x.eq_ignore_ascii_case(&item)) {
                                merged.push(item);
                            }
                        }
                        self.filter.suggestions = merged;
                    }
                    self.filter.show_suggestions = !self.filter.suggestions.is_empty();
                    self.filter.selected_suggestion = self
                        .filter
                        .selected_suggestion
                        .min(self.filter.suggestions.len().saturating_sub(1));
                    self.filter.loading_suggestions = false;
                }
            }
            CoreEvent::Error(msg) => {
                self.results.is_loading = false;
                self.error_msg = Some(msg);
                self.mutation.pending_cell_edit = None;
                self.mutation.pending_delete_row = None;
                self.filter.loading_suggestions = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_new() {
        let state = AppState::new(vec![]);
        assert_eq!(state.focused, FocusedPanel::Connections);
        assert!(!state.results.is_loading);
        assert_eq!(state.editor.mode, EditorMode::Normal);
        assert_eq!(state.vim.nav_mode, NavMode::Global);
    }

    #[test]
    fn test_app_state_navigation() {
        let mut state = AppState::new(vec![]);

        // Setup mock results
        state.results.data = QueryResult {
            columns: vec!["a".into(), "b".into()],
            rows: vec![vec!["1".into(), "2".into()], vec!["3".into(), "4".into()]],
            page: 0,
            has_next_page: false,
        };
        state.results.viewport_height = 10;
        state.results.viewport_cols = 2;

        // Move row down
        state.results.move_row_down_with_page_hint();
        assert_eq!(state.results.selected_row, 1);

        // Try to move beyond last row (should stay)
        state.results.move_row_down_with_page_hint();
        assert_eq!(state.results.selected_row, 1);

        // Move up
        state.results.move_row_up();
        assert_eq!(state.results.selected_row, 0);

        // Move col right
        state.results.move_col_right();
        assert_eq!(state.results.selected_col, 1);

        // Try to move beyond last col
        state.results.move_col_right();
        assert_eq!(state.results.selected_col, 1);

        // Move col left
        state.results.move_col_left();
        assert_eq!(state.results.selected_col, 0);
    }

    #[test]
    fn test_app_state_sort_toggle() {
        let mut state = AppState::new(vec![]);

        // Toggle ASC
        let (col, dir) = state.results.toggle_sort("id");
        assert_eq!(col, "id");
        assert_eq!(dir, Some(SortDirection::Ascending));
        assert_eq!(
            state.results.sort_state.get("id"),
            Some(&SortDirection::Ascending)
        );

        // Toggle DESC
        let (_, dir) = state.results.toggle_sort("id");
        assert_eq!(dir, Some(SortDirection::Descending));

        // Toggle OFF
        let (_, dir) = state.results.toggle_sort("id");
        assert_eq!(dir, None);
        assert!(state.results.sort_state.is_empty());
    }

    // -----------------------------------------------------------------------
    // apply_core_event tests
    // -----------------------------------------------------------------------

    #[test]
    fn core_event_connection_list() {
        let mut state = AppState::new(vec![]);
        let conns = vec![
            ConnectionConfig::new("a", "h", 5432, "u", "d"),
            ConnectionConfig::new("b", "h", 5432, "u", "d"),
        ];
        state.apply_core_event(CoreEvent::ConnectionList(conns));
        assert_eq!(state.conn.connections.len(), 2);
    }

    #[test]
    fn core_event_connection_list_clamps_selected() {
        let mut state = AppState::new(vec![
            ConnectionConfig::new("a", "h", 5432, "u", "d"),
            ConnectionConfig::new("b", "h", 5432, "u", "d"),
        ]);
        state.conn.selected = 1;
        // Now replace with just 1 connection
        state.apply_core_event(CoreEvent::ConnectionList(vec![
            ConnectionConfig::new("a", "h", 5432, "u", "d"),
        ]));
        assert_eq!(state.conn.selected, 0);
    }

    #[test]
    fn core_event_connected() {
        let mut state = AppState::new(vec![]);
        let id = Uuid::new_v4();
        state.apply_core_event(CoreEvent::Connected(id));
        assert_eq!(state.conn.active_id, Some(id));
        assert!(state.error_msg.is_none());
    }

    #[test]
    fn core_event_disconnected() {
        let mut state = AppState::new(vec![]);
        let id = Uuid::new_v4();
        state.conn.active_id = Some(id);
        state.apply_core_event(CoreEvent::Disconnected(id));
        assert!(state.conn.active_id.is_none());
        assert!(state.tables.tables.is_empty());
    }

    #[test]
    fn core_event_disconnected_different_id() {
        let mut state = AppState::new(vec![]);
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        state.conn.active_id = Some(id1);
        state.apply_core_event(CoreEvent::Disconnected(id2));
        // Should keep active since different id
        assert_eq!(state.conn.active_id, Some(id1));
    }

    #[test]
    fn core_event_table_list() {
        let mut state = AppState::new(vec![]);
        state.tables.selected = 5;
        let tables = vec![
            TableEntry { schema: "public".into(), name: "users".into() },
            TableEntry { schema: "public".into(), name: "posts".into() },
        ];
        state.apply_core_event(CoreEvent::TableList(tables));
        assert_eq!(state.tables.tables.len(), 2);
        assert_eq!(state.tables.selected, 0); // reset
    }

    #[test]
    fn core_event_query_result_page_0_resets() {
        let mut state = AppState::new(vec![]);
        state.results.selected_row = 5;
        state.results.selected_col = 3;
        state.results.scroll = 10;
        let result = QueryResult {
            columns: vec!["id".into()],
            rows: vec![vec!["1".into()]],
            page: 0,
            has_next_page: false,
        };
        state.apply_core_event(CoreEvent::QueryResult(result));
        assert_eq!(state.results.selected_row, 0);
        assert_eq!(state.results.selected_col, 0);
        assert_eq!(state.results.scroll, 0);
        assert_eq!(state.results.current_page, 0);
    }

    #[test]
    fn core_event_query_result_page_n_preserves_position() {
        let mut state = AppState::new(vec![]);
        state.results.selected_row = 5;
        state.results.selected_col = 2;
        let result = QueryResult {
            columns: vec!["id".into()],
            rows: vec![vec!["1".into()]],
            page: 2,
            has_next_page: false,
        };
        state.apply_core_event(CoreEvent::QueryResult(result));
        assert_eq!(state.results.selected_row, 5);
        assert_eq!(state.results.current_page, 2);
    }

    #[test]
    fn core_event_query_result_preserves_columns_on_empty() {
        let mut state = AppState::new(vec![]);
        state.results.data.columns = vec!["id".into(), "name".into()];
        let result = QueryResult {
            columns: vec![],
            rows: vec![],
            page: 1,
            has_next_page: false,
        };
        state.apply_core_event(CoreEvent::QueryResult(result));
        assert_eq!(state.results.data.columns, vec!["id".to_string(), "name".to_string()]);
    }

    #[test]
    fn core_event_cell_updated() {
        let mut state = AppState::new(vec![]);
        state.mutation.cell_edit = Some(CellEditState::new(
            0, 0, "id".into(), "1".into(), "public".into(), "t".into(), "id".into(), "1".into(),
        ));
        state.apply_core_event(CoreEvent::CellUpdated);
        assert!(state.mutation.cell_edit.is_none());
    }

    #[test]
    fn core_event_row_deleted() {
        let mut state = AppState::new(vec![]);
        state.results.is_loading = true;
        state.apply_core_event(CoreEvent::RowDeleted);
        assert!(!state.results.is_loading);
    }

    #[test]
    fn core_event_loading() {
        let mut state = AppState::new(vec![]);
        state.apply_core_event(CoreEvent::Loading);
        assert!(state.results.is_loading);
        assert!(state.error_msg.is_none());
    }

    #[test]
    fn core_event_primary_keys_for_delete() {
        let mut state = AppState::new(vec![]);
        state.results.data.columns = vec!["id".into(), "name".into()];
        state.results.data.rows = vec![
            vec!["1".into(), "Alice".into()],
            vec!["2".into(), "Bob".into()],
        ];
        state.mutation.pending_delete_row = Some(0);
        state.apply_core_event(CoreEvent::PrimaryKeys {
            schema: "public".into(),
            table: "users".into(),
            columns: vec!["id".into()],
        });
        assert!(state.mutation.pending_deletes.contains_key(&0));
    }

    #[test]
    fn core_event_primary_keys_for_delete_toggle() {
        let mut state = AppState::new(vec![]);
        state.results.data.columns = vec!["id".into()];
        state.results.data.rows = vec![vec!["1".into()]];
        // First: mark for deletion
        state.mutation.pending_delete_row = Some(0);
        state.apply_core_event(CoreEvent::PrimaryKeys {
            schema: "public".into(),
            table: "users".into(),
            columns: vec!["id".into()],
        });
        assert!(state.mutation.pending_deletes.contains_key(&0));

        // Second: toggle off
        state.mutation.pending_delete_row = Some(0);
        state.apply_core_event(CoreEvent::PrimaryKeys {
            schema: "public".into(),
            table: "users".into(),
            columns: vec!["id".into()],
        });
        assert!(!state.mutation.pending_deletes.contains_key(&0));
    }

    #[test]
    fn core_event_primary_keys_for_cell_edit() {
        let mut state = AppState::new(vec![]);
        state.results.data.columns = vec!["id".into(), "name".into()];
        state.results.data.rows = vec![vec!["1".into(), "Alice".into()]];
        state.mutation.pending_cell_edit = Some((0, 1));
        state.apply_core_event(CoreEvent::PrimaryKeys {
            schema: "public".into(),
            table: "users".into(),
            columns: vec!["id".into()],
        });
        assert!(state.mutation.cell_edit.is_some());
        let ce = state.mutation.cell_edit.as_ref().unwrap();
        assert_eq!(ce.col_name, "name");
        assert_eq!(ce.pk_val, "1");
    }

    #[test]
    fn core_event_primary_keys_no_pk_error() {
        let mut state = AppState::new(vec![]);
        state.results.data.columns = vec!["name".into()];
        state.results.data.rows = vec![vec!["Alice".into()]];
        state.mutation.pending_cell_edit = Some((0, 0));
        state.apply_core_event(CoreEvent::PrimaryKeys {
            schema: "public".into(),
            table: "users".into(),
            columns: vec![], // no pk
        });
        assert!(state.mutation.cell_edit.is_none());
        assert!(state.error_msg.is_some());
    }

    #[test]
    fn core_event_diagram_loaded() {
        let mut state = AppState::new(vec![]);
        let data = DiagramData::default();
        state.apply_core_event(CoreEvent::DiagramLoaded(data));
        assert!(state.diagram.is_some());
    }

    #[test]
    fn core_event_filter_suggestions_matching_token() {
        let mut state = AppState::new(vec![]);
        state.filter.visible = true;
        state.filter.suggestion_token = 5;
        state.filter.suggestions = vec!["existing".into()];
        state.apply_core_event(CoreEvent::FilterSuggestions {
            items: vec!["new_item".into()],
            token: 5,
        });
        assert!(state.filter.suggestions.contains(&"new_item".to_string()));
        assert!(!state.filter.loading_suggestions);
    }

    #[test]
    fn core_event_filter_suggestions_stale_token() {
        let mut state = AppState::new(vec![]);
        state.filter.visible = true;
        state.filter.suggestion_token = 5;
        state.filter.suggestions = vec!["existing".into()];
        state.apply_core_event(CoreEvent::FilterSuggestions {
            items: vec!["stale".into()],
            token: 3, // old token
        });
        // Should NOT merge stale items
        assert!(!state.filter.suggestions.contains(&"stale".to_string()));
    }

    #[test]
    fn core_event_error() {
        let mut state = AppState::new(vec![]);
        state.results.is_loading = true;
        state.apply_core_event(CoreEvent::Error("something failed".into()));
        assert!(!state.results.is_loading);
        assert_eq!(state.error_msg, Some("something failed".into()));
    }

    // -----------------------------------------------------------------------
    // ConnectionForm tests
    // -----------------------------------------------------------------------

    #[test]
    fn connection_form_field_labels() {
        let form = ConnectionForm::default(); // Postgres backend
        assert_eq!(form.field_label(0), "Backend");
        assert_eq!(form.field_label(1), "Name");
        assert_eq!(form.field_label(7), "SSL Mode");
        assert_eq!(form.field_label(8), "");
    }

    #[test]
    fn connection_form_cycle_ssl() {
        let mut form = ConnectionForm::default();
        form.cycle_ssl_mode();
        assert_eq!(form.ssl_mode, SslMode::Require);
        form.cycle_ssl_mode();
        assert_eq!(form.ssl_mode, SslMode::VerifyFull);
        form.cycle_ssl_mode();
        assert_eq!(form.ssl_mode, SslMode::VerifyCa);
        form.cycle_ssl_mode();
        assert_eq!(form.ssl_mode, SslMode::Disable);
        form.cycle_ssl_mode();
        assert_eq!(form.ssl_mode, SslMode::Prefer);
    }

    #[test]
    fn connection_form_active_value_mut() {
        let mut form = ConnectionForm::default();
        form.field_index = 0;
        assert!(form.active_value_mut().is_none()); // Backend is cycled

        form.field_index = 1;
        *form.active_value_mut().unwrap() = "test".into();
        assert_eq!(form.name, "test");

        form.field_index = 7;
        assert!(form.active_value_mut().is_none()); // SSL mode is cycled
    }

    #[test]
    fn connection_form_open_new() {
        let form = ConnectionForm::open_new();
        assert!(form.visible);
        assert_eq!(form.port, "5432");
        assert_eq!(form.host, "localhost");
    }

    #[test]
    fn connection_form_open_edit() {
        let cfg = ConnectionConfig::new("myconn", "myhost", 3333, "myuser", "mydb");
        let form = ConnectionForm::open_edit(&cfg);
        assert!(form.visible);
        assert_eq!(form.name, "myconn");
        assert_eq!(form.host, "myhost");
        assert_eq!(form.port, "3333");
        assert!(form.editing_id.is_some());
    }

    // -----------------------------------------------------------------------
    // MutationState tests
    // -----------------------------------------------------------------------

    #[test]
    fn mutation_state_discard_pending() {
        let mut ms = MutationState {
            cell_edit: None,
            pending_cell_edit: None,
            pending_edits: HashMap::new(),
            pending_deletes: HashMap::new(),
            pending_delete_row: Some(3),
            pending_d: true,
        };
        ms.pending_edits.insert((0, 0), PendingEdit {
            new_val: "x".into(),
            original: "y".into(),
            schema: "p".into(),
            table: "t".into(),
            pk_col: "id".into(),
            pk_val: "1".into(),
            col_name: "c".into(),
        });
        ms.discard_pending();
        assert!(ms.pending_edits.is_empty());
        assert!(ms.pending_deletes.is_empty());
        assert!(ms.pending_delete_row.is_none());
        assert!(!ms.pending_d);
    }

    // -----------------------------------------------------------------------
    // ResultsState navigation edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn results_move_row_down_empty() {
        let mut state = AppState::new(vec![]);
        let hint = state.results.move_row_down_with_page_hint();
        assert!(!hint);
    }

    #[test]
    fn results_move_row_up_at_zero() {
        let mut state = AppState::new(vec![]);
        state.results.selected_row = 0;
        state.results.move_row_up();
        assert_eq!(state.results.selected_row, 0);
    }

    #[test]
    fn results_selected_column_name() {
        let mut state = AppState::new(vec![]);
        state.results.data.columns = vec!["id".into(), "name".into()];
        state.results.selected_col = 1;
        assert_eq!(state.results.selected_column_name(), Some("name"));
    }

    #[test]
    fn results_selected_column_name_empty() {
        let state = AppState::new(vec![]);
        assert_eq!(state.results.selected_column_name(), None);
    }

    #[test]
    fn results_half_page_down_empty() {
        let mut state = AppState::new(vec![]);
        assert!(!state.results.move_row_half_page_down());
    }

    #[test]
    fn results_half_page_down_triggers_next_page() {
        let mut state = AppState::new(vec![]);
        state.results.data.rows = vec![vec!["1".into()], vec!["2".into()]];
        state.results.data.has_next_page = true;
        state.results.viewport_height = 2;
        state.results.selected_row = 1;
        let hint = state.results.move_row_half_page_down();
        assert!(hint); // should signal next page
    }
}
