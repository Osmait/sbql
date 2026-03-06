use std::collections::HashMap;
use std::time::Instant;

use ratatui::layout::Rect;
use sbql_core::{
    ConnectionConfig, CoreEvent, DiagramData, QueryResult, SortDirection, SslMode, TableEntry,
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
    pub field_index: usize,       // which field is active (0-6)
    pub name: String,
    pub host: String,
    pub port: String,
    pub user: String,
    pub database: String,
    pub password: String,
    pub ssl_mode: SslMode,
    pub error: Option<String>,
}

impl Default for ConnectionForm {
    fn default() -> Self {
        Self {
            visible: false,
            editing_id: None,
            field_index: 0,
            name: String::new(),
            host: String::new(),
            port: String::new(),
            user: String::new(),
            database: String::new(),
            password: String::new(),
            ssl_mode: SslMode::Prefer,
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
            name: cfg.name.clone(),
            host: cfg.host.clone(),
            port: cfg.port.to_string(),
            user: cfg.user.clone(),
            database: cfg.database.clone(),
            password: String::new(), // always re-enter
            ssl_mode: cfg.ssl_mode.clone(),
            field_index: 0,
            error: None,
        }
    }

    /// Returns the label for each field index.
    pub fn field_label(idx: usize) -> &'static str {
        match idx {
            0 => "Name",
            1 => "Host",
            2 => "Port",
            3 => "User",
            4 => "Database",
            5 => "Password",
            6 => "SSL Mode",
            _ => "",
        }
    }

    pub fn field_count() -> usize {
        7
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
        match self.field_index {
            0 => Some(&mut self.name),
            1 => Some(&mut self.host),
            2 => Some(&mut self.port),
            3 => Some(&mut self.user),
            4 => Some(&mut self.database),
            5 => Some(&mut self.password),
            6 => None, // SSL mode is cycled, not typed
            _ => Some(&mut self.name),
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
        }
    }
}

// ---------------------------------------------------------------------------
// Main application state
// ---------------------------------------------------------------------------

pub struct AppState {
    // ---- panels ----
    pub focused: FocusedPanel,

    // ---- connections ----
    pub connections: Vec<ConnectionConfig>,
    pub selected_connection: usize, // index in `connections`
    pub active_connection_id: Option<Uuid>,
    pub tables: Vec<TableEntry>,
    pub selected_table: usize,
    pub conn_form: ConnectionForm,

    // ---- editor ----
    pub editor: TextArea<'static>,

    // ---- results ----
    pub results: QueryResult,
    pub result_scroll: usize,     // row offset within current page
    pub result_col_scroll: usize, // horizontal column offset
    pub selected_row: usize,
    pub selected_col: usize,
    pub current_page: usize,
    pub is_loading: bool,
    pub sort_state: HashMap<String, SortDirection>,
    /// Height of the results viewport in rows (updated each draw cycle).
    pub results_viewport_height: usize,
    /// Number of visible columns in the results viewport (updated each draw cycle).
    pub results_viewport_cols: usize,

    // ---- cell editing ----
    pub cell_edit: Option<CellEditState>,
    /// Pending cell edit: (row_idx, col_idx) awaiting a PrimaryKeys event.
    pub pending_cell_edit: Option<(usize, usize)>,

    // ---- filter ----
    pub filter_bar: FilterBar,
    /// The currently active filter string (set locally when filter is applied).
    pub active_filter: Option<String>,

    // ---- status / error ----
    pub status_msg: Option<String>,
    pub error_msg: Option<String>,
    /// Pending connection deletion confirmation: (id, display name).
    pub pending_connection_delete: Option<(Uuid, String)>,

    // ---- quit ----
    pub should_quit: bool,

    // ---- diagram mode ----
    /// When Some, the diagram full-screen overlay is active.
    pub diagram: Option<DiagramState>,

    // ---- editor mode (Vim Normal / Insert) ----
    /// Current global mode (Vim-like): Normal for navigation, Insert for typing.
    pub editor_mode: EditorMode,
    /// Global navigation scope: either panel-to-panel movement or inside-panel controls.
    pub nav_mode: NavMode,
    /// When true, the leader key (Space) was pressed in Normal mode and the
    /// next key will be interpreted as a leader combo.
    pub pending_leader: bool,

    // ---- pending vim prefix ----
    /// Set to true when the user pressed `g` in results/connections, waiting
    /// for a second `g` to complete the `gg` (go-to-top) motion.
    pub pending_g: bool,
    /// Set to true when the user pressed `d` in results, waiting for a second
    /// `d` to complete the `dd` (mark row for deletion) motion.
    pub pending_d: bool,

    // ---- staged (uncommitted) changes ----
    /// Cell edits staged with Ctrl+S, not yet sent to the DB.
    /// Key: (absolute_row_idx, col_idx) within the current page.
    pub pending_edits: HashMap<(usize, usize), PendingEdit>,
    /// Rows marked for deletion with `dd`. Key: absolute row index.
    /// Value has the resolved PK so we can issue the DELETE at commit time.
    pub pending_deletes: HashMap<usize, PendingDelete>,
    /// Row index waiting for a PrimaryKeys response before being added to pending_deletes.
    pub pending_delete_row: Option<usize>,

    // ---- last drawn layout (used for mouse hit-testing and cell edit positioning) ----
    pub last_areas: Option<LastAreas>,
    /// Column pixel widths from the last draw cycle (used to position the cell-edit overlay).
    pub last_col_widths: Vec<u16>,
    /// Current spinner animation frame index (incremented on every Tick event).
    pub spinner_frame: usize,
    /// When true the connections/tables sidebar is hidden for a full-width data view.
    pub sidebar_hidden: bool,
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
        let mut editor = TextArea::default();
        editor.set_placeholder_text("-- Write SQL here. Press Ctrl+S or F5 to run.");

        Self {
            focused: FocusedPanel::Connections,
            connections,
            selected_connection: 0,
            active_connection_id: None,
            tables: Vec::new(),
            selected_table: 0,
            conn_form: ConnectionForm::default(),
            editor,
            results: QueryResult::default(),
            result_scroll: 0,
            result_col_scroll: 0,
            selected_row: 0,
            selected_col: 0,
            current_page: 0,
            is_loading: false,
            sort_state: HashMap::new(),
            results_viewport_height: 20,
            results_viewport_cols: 5,
            cell_edit: None,
            pending_cell_edit: None,
            filter_bar: FilterBar::default(),
            active_filter: None,
            status_msg: None,
            error_msg: None,
            pending_connection_delete: None,
            should_quit: false,
            diagram: None,
            editor_mode: EditorMode::Normal,
            nav_mode: NavMode::Global,
            pending_leader: false,
            pending_g: false,
            pending_d: false,
            pending_edits: HashMap::new(),
            pending_deletes: HashMap::new(),
            pending_delete_row: None,
            last_areas: None,
            last_col_widths: Vec::new(),
            spinner_frame: 0,
            sidebar_hidden: false,
        }
    }

    // -----------------------------------------------------------------------
    // Event application
    // -----------------------------------------------------------------------

    /// Apply an incoming [`CoreEvent`] to the application state.
    pub fn apply_core_event(&mut self, event: CoreEvent) {
        match event {
            CoreEvent::ConnectionList(conns) => {
                self.is_loading = false;
                self.connections = conns;
                if self.selected_connection >= self.connections.len() {
                    self.selected_connection = self.connections.len().saturating_sub(1);
                }
            }
            CoreEvent::Connected(id) => {
                self.is_loading = false;
                self.active_connection_id = Some(id);
                let name = self
                    .connections
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| id.to_string());
                self.status_msg = Some(format!("Connected to {name}"));
                self.error_msg = None;
            }
            CoreEvent::Disconnected(id) => {
                self.is_loading = false;
                if self.active_connection_id == Some(id) {
                    self.active_connection_id = None;
                }
                self.tables.clear();
                self.status_msg = Some("Disconnected".into());
            }
            CoreEvent::TableList(tables) => {
                self.is_loading = false;
                self.tables = tables;
                self.selected_table = 0;
            }
            CoreEvent::QueryResult(result) => {
                self.is_loading = false;
                self.current_page = result.page;
                tracing::info!(
                    "QueryResult: page={} rows={} cols={} has_next={}",
                    result.page,
                    result.rows.len(),
                    result.columns.len(),
                    result.has_next_page,
                );
                if result.page == 0 {
                    // Fresh query — reset scroll and selection, but do NOT clear
                    // sort_state here: toggle_sort() and the ApplyOrder/ClearOrder
                    // handlers own it. Clearing here would wipe the sort indicator
                    // the moment sorted results arrive.
                    self.result_scroll = 0;
                    self.result_col_scroll = 0;
                    self.selected_row = 0;
                    self.selected_col = 0;
                }
                // Always discard staged changes when a new result set arrives
                // (covers fresh query, page change, and post-commit refresh).
                self.discard_pending();
                self.results = result;
                self.error_msg = None;
                // Clear any stale status message now that we have fresh results
                self.status_msg = None;
            }
            CoreEvent::CellUpdated => {
                self.is_loading = false;
                self.cell_edit = None;
            }
            CoreEvent::RowDeleted => {
                self.is_loading = false;
            }
            CoreEvent::PrimaryKeys {
                schema,
                table,
                columns,
            } => {
                self.is_loading = false;
                let pk_col = columns.into_iter().next().unwrap_or_default();

                // Resolve a pending delete if one is waiting for this PK info.
                if let Some(row_idx) = self.pending_delete_row.take() {
                    let pk_val = self
                        .results
                        .columns
                        .iter()
                        .position(|c| c.to_lowercase() == pk_col.to_lowercase())
                        .and_then(|pk_ci| {
                            self.results
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
                        if self.pending_deletes.contains_key(&row_idx) {
                            self.pending_deletes.remove(&row_idx);
                        } else {
                            self.pending_deletes.insert(
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
                if let Some((row_idx, col_idx)) = self.pending_cell_edit.take() {
                    let col_name = match self.results.columns.get(col_idx) {
                        Some(c) => c.clone(),
                        None => return,
                    };
                    let original = self
                        .results
                        .rows
                        .get(row_idx)
                        .and_then(|r| r.get(col_idx))
                        .cloned()
                        .unwrap_or_default();
                    // Find PK value in result row
                    let pk_val = self
                        .results
                        .columns
                        .iter()
                        .position(|c| c.to_lowercase() == pk_col.to_lowercase())
                        .and_then(|pk_ci| {
                            self.results
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

                    self.cell_edit = Some(CellEditState::new(
                        row_idx, col_idx, col_name, original, schema, table, pk_col, pk_val,
                    ));
                }
            }
            CoreEvent::Loading => {
                self.is_loading = true;
                self.error_msg = None;
            }
            CoreEvent::DiagramLoaded(data) => {
                self.is_loading = false;
                self.diagram = Some(DiagramState::new(data));
            }
            CoreEvent::FilterSuggestions { items, token } => {
                self.is_loading = false;
                if self.filter_bar.visible && token == self.filter_bar.suggestion_token {
                    if !items.is_empty() {
                        let mut merged = self.filter_bar.suggestions.clone();
                        for item in items {
                            if !merged.iter().any(|x| x.eq_ignore_ascii_case(&item)) {
                                merged.push(item);
                            }
                        }
                        self.filter_bar.suggestions = merged;
                    }
                    self.filter_bar.show_suggestions = !self.filter_bar.suggestions.is_empty();
                    self.filter_bar.selected_suggestion = self
                        .filter_bar
                        .selected_suggestion
                        .min(self.filter_bar.suggestions.len().saturating_sub(1));
                    self.filter_bar.loading_suggestions = false;
                }
            }
            CoreEvent::Error(msg) => {
                self.is_loading = false;
                self.error_msg = Some(msg);
                // Clean up any pending state that was waiting for a Core response.
                // Without this, a failed GetPrimaryKeys leaves pending_cell_edit /
                // pending_delete_row dangling and the next `i` keypress misbehaves.
                self.pending_cell_edit = None;
                self.pending_delete_row = None;
                self.filter_bar.loading_suggestions = false;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Navigation helpers
    // -----------------------------------------------------------------------

    pub fn move_row_down_with_page_hint(&mut self) -> bool {
        let len = self.results.rows.len();
        if len == 0 {
            return false;
        }
        let max = len - 1;
        if self.selected_row < max {
            self.selected_row += 1;
            self.clamp_scroll();
            false
        } else if self.results.has_next_page {
            // Already at last row of page — signal caller to fetch next page
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

    /// Adjust `result_scroll` so `selected_row` stays within the viewport.
    fn clamp_scroll(&mut self) {
        let vh = self.results_viewport_height.max(1);
        if self.selected_row < self.result_scroll {
            self.result_scroll = self.selected_row;
        } else if self.selected_row >= self.result_scroll + vh {
            self.result_scroll = self.selected_row + 1 - vh;
        }
    }

    pub fn move_col_right(&mut self) {
        let max = self.results.columns.len().saturating_sub(1);
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
        let len = self.results.rows.len();
        if len > 0 {
            self.selected_row = len - 1;
            self.clamp_scroll();
        }
    }

    /// Move down by half the viewport height (vim `Ctrl+d`).
    pub fn move_row_half_page_down(&mut self) -> bool {
        let half = (self.results_viewport_height / 2).max(1);
        let len = self.results.rows.len();
        if len == 0 {
            return false;
        }
        let max = len - 1;
        if self.selected_row + half <= max {
            self.selected_row += half;
            self.clamp_scroll();
            false
        } else if self.results.has_next_page {
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
        let half = (self.results_viewport_height / 2).max(1);
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
        let max = self.results.columns.len().saturating_sub(1);
        self.selected_col = max;
        self.clamp_col_scroll();
    }

    /// Adjust `result_col_scroll` so `selected_col` stays within the visible
    /// column window. `results_viewport_cols` is set by the draw cycle.
    fn clamp_col_scroll(&mut self) {
        let vc = self.results_viewport_cols.max(1);
        if self.selected_col < self.result_col_scroll {
            self.result_col_scroll = self.selected_col;
        } else if self.selected_col >= self.result_col_scroll + vc {
            self.result_col_scroll = self.selected_col + 1 - vc;
        }
    }

    /// Return the column name under the current cursor.
    pub fn selected_column_name(&self) -> Option<&str> {
        self.results
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

    /// Get the current SQL text from the editor.
    pub fn editor_sql(&self) -> String {
        self.editor.lines().join("\n")
    }

    /// Discard all staged (uncommitted) edits and deletes.
    pub fn discard_pending(&mut self) {
        self.pending_edits.clear();
        self.pending_deletes.clear();
        self.pending_delete_row = None;
        self.pending_d = false;
    }
}
