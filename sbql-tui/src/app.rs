use std::collections::HashMap;
use std::time::Instant;

use ratatui::layout::Rect;
use sbql_core::{
    ConnectionConfig, ConnectionDraft, CoreEvent, DbBackend, DiagramData, DiscoveredConnection,
    FieldSpec, QueryResult, SortDirection, SslMode, TableEntry,
};
use tui_textarea::TextArea;
use uuid::Uuid;

use crate::completion::CompletionState;
use crate::highlight::SqlHighlighter;
use crate::list_cursor::ListCursor;
use crate::notice::Notice;
use crate::ui::hit::HitMap;

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
    /// The connection being typed. All field data, validation and the
    /// backend's field list come from `sbql-core`, so the TUI and the macOS
    /// app cannot drift apart on what a connection needs.
    pub draft: ConnectionDraft,
    /// Row 0 is the backend picker; rows 1.. map onto `spec().fields`.
    pub field_index: usize,
    pub error: Option<String>,
}

impl Default for ConnectionForm {
    fn default() -> Self {
        Self {
            visible: false,
            draft: ConnectionDraft::new(DbBackend::Postgres),
            field_index: 0,
            error: None,
        }
    }
}

impl ConnectionForm {
    pub fn open_new() -> Self {
        Self {
            visible: true,
            ..Default::default()
        }
    }

    pub fn open_edit(cfg: &ConnectionConfig) -> Self {
        Self {
            visible: true,
            draft: ConnectionDraft::from_config(cfg),
            field_index: 0,
            error: None,
        }
    }

    /// One row for the backend picker plus one per field the backend declares.
    pub fn field_count(&self) -> usize {
        1 + self.draft.spec().fields.len()
    }

    /// Which connection field a row edits. `None` for the backend picker.
    pub fn field_at(&self, idx: usize) -> Option<&'static FieldSpec> {
        if idx == 0 {
            return None;
        }
        self.draft.spec().fields.get(idx - 1)
    }

    pub fn field_label(&self, idx: usize) -> &'static str {
        match self.field_at(idx) {
            _ if idx == 0 => "Backend",
            Some(spec) => spec.label,
            None => "",
        }
    }

    /// The active row's value, when it is one that is typed into.
    pub fn active_value_mut(&mut self) -> Option<&mut String> {
        let field = self.field_at(self.field_index)?.field;
        self.draft.value_mut(field)
    }

    /// The row showing a given field, so a validation error can move the
    /// cursor to the field it is complaining about.
    pub fn row_of(&self, field: sbql_core::ConnectionField) -> Option<usize> {
        self.draft
            .spec()
            .fields
            .iter()
            .position(|f| f.field == field)
            .map(|i| i + 1)
    }

    pub fn cycle_backend(&mut self) {
        self.draft.set_backend(self.draft.backend.next());
        self.field_index = 0;
    }

    /// Cycle through SSL mode options (for the SSL Mode field).
    pub fn cycle_ssl_mode(&mut self) {
        self.draft.ssl_mode = match self.draft.ssl_mode {
            SslMode::Prefer => SslMode::Require,
            SslMode::Require => SslMode::VerifyFull,
            SslMode::VerifyFull => SslMode::VerifyCa,
            SslMode::VerifyCa => SslMode::Disable,
            SslMode::Disable => SslMode::Prefer,
        };
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
    /// Every `(column, value)` component of the row's primary key.
    pub pk: Vec<(String, String)>,
    pub textarea: TextArea<'static>,
}

impl CellEditState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        row_idx: usize,
        col_idx: usize,
        col_name: String,
        original: String,
        schema: String,
        table: String,
        pk: Vec<(String, String)>,
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
            pk,
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

#[derive(Debug, Default)]
pub struct FilterBar {
    pub visible: bool,
    pub textarea: TextArea<'static>,
    pub suggestions: Vec<String>,
    pub suggestion_cursor: ListCursor,
    pub show_suggestions: bool,
    pub suggestion_token: u64,
    pub loading_suggestions: bool,
    pub pending_live_apply_at: Option<Instant>,
    pub last_applied_query: Option<String>,
}

// ---------------------------------------------------------------------------
// Pending (staged) edit model
// ---------------------------------------------------------------------------

/// A cell edit that has been staged locally but not yet committed to the DB.
#[derive(Debug, Clone)]
pub struct PendingEdit {
    pub new_val: String,
    pub schema: String,
    pub table: String,
    /// Every `(column, value)` component of the row's primary key. A composite
    /// key reduced to its first column made the UPDATE hit every row sharing
    /// that component.
    pub pk: Vec<(String, String)>,
    pub col_name: String,
}

/// A row marked for deletion, with its full PK already resolved.
#[derive(Debug, Clone)]
pub struct PendingDelete {
    pub schema: String,
    pub table: String,
    pub pk: Vec<(String, String)>,
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
    /// Databases found running in Docker this session. Offered below the saved
    /// ones and never written to disk unless the user asks.
    pub discovered: Vec<DiscoveredConnection>,
    pub cursor: ListCursor,
    pub active_id: Option<Uuid>,
    pub active_backend: DbBackend,
    pub form: ConnectionForm,
    pub pending_delete: Option<(Uuid, String)>,
}

/// One row of the connections panel.
///
/// The panel shows two lists that behave differently — saved connections can be
/// edited and deleted, discovered ones can only be connected to or saved — but
/// the cursor moves through them as one. Making that a type means every call
/// site has to say which kind it is looking at instead of assuming.
#[derive(Debug, Clone, Copy)]
pub enum ConnectionEntry<'a> {
    Saved(&'a ConnectionConfig),
    Discovered(&'a DiscoveredConnection),
}

impl<'a> ConnectionEntry<'a> {
    pub fn config(self) -> &'a ConnectionConfig {
        match self {
            ConnectionEntry::Saved(c) => c,
            ConnectionEntry::Discovered(d) => &d.config,
        }
    }

    pub fn is_discovered(self) -> bool {
        matches!(self, ConnectionEntry::Discovered(_))
    }
}

impl ConnectionState {
    pub fn selected(&self) -> usize {
        self.cursor.index()
    }

    /// Every row the panel draws, saved first.
    pub fn entries(&self) -> impl Iterator<Item = ConnectionEntry<'_>> {
        self.connections
            .iter()
            .map(ConnectionEntry::Saved)
            .chain(self.discovered.iter().map(ConnectionEntry::Discovered))
    }

    /// How many rows the cursor can land on. Not `connections.len()` — that
    /// would make the discovered ones unreachable.
    pub fn len(&self) -> usize {
        self.connections.len() + self.discovered.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn entry(&self, idx: usize) -> Option<ConnectionEntry<'_>> {
        self.entries().nth(idx)
    }

    /// The row the cursor is on.
    pub fn selected_entry(&self) -> Option<ConnectionEntry<'_>> {
        self.entry(self.selected())
    }
}

pub struct TableBrowserState {
    pub tables: Vec<TableEntry>,
    pub cursor: ListCursor,
}

impl TableBrowserState {
    pub fn selected(&self) -> usize {
        self.cursor.index()
    }
}

pub struct EditorState {
    pub textarea: TextArea<'static>,
    pub mode: EditorMode,
    // Syntax highlighting
    pub scroll_row: usize,
    pub scroll_col: usize,
    /// Bumped on every text change. The view keys its highlight cache on this
    /// so the model never holds rendered output.
    pub revision: u64,
    pub highlighter: SqlHighlighter,
    // Autocomplete
    pub completion: CompletionState,
    /// Whether a mouse drag is currently extending a selection.
    ///
    /// The selection anchor is dropped on the first drag event rather than on
    /// mouse-down, so an ordinary click does not leave a live selection behind.
    pub dragging: bool,
}

impl EditorState {
    /// Get the current SQL text from the editor.
    pub fn sql(&self) -> String {
        self.textarea.lines().join("\n")
    }

    /// Record that the text changed, so the view knows to re-highlight.
    pub fn mark_text_changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
}

/// Where the cursor goes when a page of results arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Landing {
    /// The first row: a new query, or the next page while reading forward.
    Top,
    /// The last row: the previous page, reached by paging back.
    Bottom,
    /// Wherever it was: the same page again, so the user has not moved.
    Keep,
}

#[derive(Default)]
pub struct ResultsState {
    pub data: QueryResult,
    /// The SQL whose execution produced `data` — what row edits and deletes
    /// resolve their target table from. The editor text is *not* usable for
    /// that: the user may have typed a new query without running it.
    pub source_sql: Option<String>,
    /// Set when a new query is sent, cleared when its result arrives.
    ///
    /// Page numbers alone cannot tell a fresh query from paging back to the
    /// first page — both deliver page 0 — and the two want opposite ends of
    /// it, so the client records which one it asked for.
    pub awaiting_new_query: bool,
    /// The SQL most recently sent for execution; promoted to `source_sql`
    /// when its result actually arrives.
    pub sent_sql: Option<String>,
    pub scroll: usize,
    pub col_scroll: usize,
    pub selected_row: usize,
    pub selected_col: usize,
    pub current_page: usize,
    pub is_loading: bool,
    /// The sort core last said it was applying — a cache, never a decision.
    ///
    /// Written only from [`CoreEvent::SortChanged`]. The TUI used to keep its
    /// own sort map and update it optimistically on the `o` key, which drifted
    /// the moment core dropped the sort on its own: after a disconnect (or
    /// editing a connection's target) the header still drew an arrow for an
    /// ORDER BY no query was applying.
    pub sort: Option<(String, SortDirection)>,
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
        } else {
            self.data.has_next_page
        }
    }

    /// Move up one row, or ask for the previous page.
    ///
    /// Returns whether the caller should fetch it. Without this, landing on
    /// the first row of a page was a dead end: the rows above were only
    /// reachable by leaving the table and opening it again.
    pub fn move_row_up_with_page_hint(&mut self) -> bool {
        if self.selected_row > 0 {
            self.selected_row -= 1;
            self.clamp_scroll();
            false
        } else {
            self.current_page > 0
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
    /// Move up half a screen, or ask for the previous page when already at
    /// the top. Mirrors [`Self::move_row_half_page_down`].
    pub fn move_row_half_page_up(&mut self) -> bool {
        if self.selected_row == 0 {
            return self.current_page > 0;
        }
        let half = (self.viewport_height / 2).max(1);
        self.selected_row = self.selected_row.saturating_sub(half);
        self.clamp_scroll();
        false
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
        self.data.columns.get(self.selected_col).map(String::as_str)
    }

    /// The sort direction the `o` key should ask core for next on `col`.
    /// Cycles None → Asc → Desc → None; `None` means "clear the sort".
    ///
    /// Read-only on purpose. The cycle is computed from what core last
    /// reported, and the answer is sent as a command — writing the new sort
    /// locally as well is what let the cached sort survive a disconnect that
    /// had already dropped it.
    pub fn next_sort_direction(&self, col: &str) -> Option<SortDirection> {
        match &self.sort {
            Some((sorted, SortDirection::Ascending)) if sorted == col => {
                Some(SortDirection::Descending)
            }
            Some((sorted, SortDirection::Descending)) if sorted == col => None,
            // Unsorted, or sorted by a different column: start the cycle over.
            _ => Some(SortDirection::Ascending),
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

/// The theme picker overlay.
///
/// Holds what to go back to: moving the cursor applies a theme immediately so
/// the whole UI is the preview, and Esc has to be able to undo that.
pub struct ThemePicker {
    pub visible: bool,
    pub cursor: ListCursor,
    /// The theme that was in use when the picker opened.
    pub previous: usize,
}

impl Default for ThemePicker {
    fn default() -> Self {
        Self {
            visible: false,
            cursor: ListCursor::new(),
            previous: 0,
        }
    }
}

impl ThemePicker {
    /// Open on the theme in use, so the list starts where the eye is.
    pub fn open() -> Self {
        let current = crate::ui::theme::current_index();
        let mut cursor = ListCursor::new();
        cursor.select(current, crate::ui::theme::THEMES.len());
        Self {
            visible: true,
            cursor,
            previous: current,
        }
    }

    pub fn selected(&self) -> usize {
        self.cursor.index()
    }
}

pub struct VimState {
    pub nav_mode: NavMode,
    pub pending_leader: bool,
    pub pending_g: bool,
}

pub struct LayoutCache {
    /// The results grid's rect from the last draw.
    ///
    /// The only panel rect still needed outside drawing: the cell-edit popup
    /// floats over the selected cell, so it has to know where the grid was.
    /// Everything else that used to be cached here is answered by `hits`.
    pub results_area: Option<Rect>,
    pub last_col_widths: Vec<u16>,
    pub spinner_frame: usize,
    pub sidebar_hidden: bool,
    /// When false, skip the terminal.draw() call to avoid redundant repaints.
    pub needs_redraw: bool,
    /// What the last frame painted where, so a click can find it.
    pub hits: HitMap,
    /// The editor's text region from the last frame.
    ///
    /// Held apart from the hit map because a drag has to keep addressing the
    /// editor after the pointer has left it — the selection should follow the
    /// mouse past the edge, as it does everywhere else.
    pub editor_text_rect: Option<Rect>,
    /// The pointer's position during a drag, for panning by delta.
    pub last_drag: Option<(u16, u16)>,
    /// Where and when the last left-click landed: `(col, row, tick)`.
    ///
    /// Double-click is measured in ticks rather than wall-clock so the mouse
    /// handler stays a pure function of state — the same reason notices expire
    /// on ticks instead of reading a clock.
    pub last_click: Option<(u16, u16, u64)>,
}

/// Ticks within which a second click at the same spot is a double-click.
///
/// The loop ticks every 100ms, so this is 400ms — the usual system default.
const DOUBLE_CLICK_TICKS: u64 = 4;

impl LayoutCache {
    /// Whether a click at this point continues the previous one.
    ///
    /// Same cell, close enough in time. Requiring the exact cell rather than a
    /// neighbourhood is deliberate: a terminal cell is already a large target,
    /// and a drifting double-click that acts on the row *next* to the one the
    /// user pointed at is worse than no double-click at all.
    pub fn is_double_click(&self, col: u16, row: u16, tick: u64) -> bool {
        self.last_click.is_some_and(|(c, r, t)| {
            c == col && r == row && tick.wrapping_sub(t) <= DOUBLE_CLICK_TICKS
        })
    }
}

// ---------------------------------------------------------------------------
// Main application state
// ---------------------------------------------------------------------------

/// Which part of the UI currently owns the keyboard.
///
/// Overlays are exclusive: opening one closes any other. The variants are
/// listed in precedence order, which is the order [`AppState::mode`] resolves
/// them — so the answer to "who gets this key" is declared here rather than
/// implied by the order of a chain of `if`s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Full-screen schema diagram. Takes every key.
    Diagram,
    /// The full text of the current notice, over the workspace.
    NoticeDetail,
    /// The theme picker.
    ThemePicker,
    /// Single-cell edit popup over the results grid.
    CellEdit,
    /// Filter bar under the results grid.
    Filter,
    /// Add/edit connection dialog.
    ConnectionForm,
    /// "Delete connection?" confirmation.
    ConfirmDelete,
    /// No overlay: keys go to the focused panel.
    Browsing,
}

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
    /// True when the user explicitly requested the diagram overlay (Shift+D).
    /// Prevents auto-loaded diagram data from opening the overlay.
    pub diagram_requested: bool,

    // ---- cached diagram data for completions ----
    pub cached_diagram: Option<DiagramData>,

    // ---- what to tell the user ----
    /// The one thing the status bar is saying, if anything.
    pub notice: Option<Notice>,
    /// Whether the full text of `notice` is open over the workspace.
    pub notice_detail_open: bool,
    /// The theme picker overlay.
    pub theme_picker: ThemePicker,
    /// Ticks since startup. Only used to expire notices; see [`Notice`].
    pub tick: u64,

    // ---- quit ----
    pub should_quit: bool,
}

impl AppState {
    /// Which mode owns the keyboard right now.
    ///
    /// The single place overlay precedence is decided. Callers `match` on the
    /// result, so adding an overlay forces every dispatch site to say what it
    /// does with it, instead of silently falling through to the panel keys.
    pub fn mode(&self) -> Mode {
        if self.diagram.is_some() {
            Mode::Diagram
        } else if self.theme_picker.visible {
            Mode::ThemePicker
        } else if self.notice_detail_open {
            Mode::NoticeDetail
        } else if self.mutation.cell_edit.is_some() {
            Mode::CellEdit
        } else if self.filter.visible {
            Mode::Filter
        } else if self.conn.form.visible {
            Mode::ConnectionForm
        } else if self.conn.pending_delete.is_some() {
            Mode::ConfirmDelete
        } else {
            Mode::Browsing
        }
    }

    /// Close every overlay.
    ///
    /// Opening an overlay goes through here first, so two can never be open at
    /// once and `mode()` never has to arbitrate between contradictory state.
    pub fn close_overlays(&mut self) {
        self.diagram = None;
        self.diagram_requested = false;
        self.mutation.cell_edit = None;
        self.filter.visible = false;
        self.filter.show_suggestions = false;
        self.conn.form.visible = false;
        self.conn.pending_delete = None;
        self.notice_detail_open = false;
        self.theme_picker.visible = false;
    }

    // -----------------------------------------------------------------------
    // Telling the user things
    // -----------------------------------------------------------------------

    /// Confirm that something worked. Clears itself after a few seconds.
    pub fn inform(&mut self, text: impl Into<String>) {
        self.notice = Some(Notice::info(text, self.tick));
    }

    /// Report a failure the TUI worked out on its own.
    pub fn report(&mut self, text: impl Into<String>) {
        self.notice = Some(Notice::error(text, self.tick));
    }

    /// Report what the core sent back, keeping its kind, severity and cause.
    pub fn report_core(&mut self, err: sbql_core::CoreError) {
        self.notice = Some(Notice::from_core(err, self.tick));
    }

    /// Take the message down.
    pub fn dismiss_notice(&mut self) {
        self.notice = None;
        self.notice_detail_open = false;
    }

    /// What the status bar is saying, if anything.
    #[cfg(test)]
    pub fn notice_text(&self) -> Option<&str> {
        self.notice.as_ref().map(|n| n.text.as_str())
    }

    /// Whether the bar is currently showing a failure (not a warning, not a
    /// confirmation).
    #[cfg(test)]
    pub fn is_failing(&self) -> bool {
        self.notice
            .as_ref()
            .is_some_and(|n| n.level == crate::notice::Level::Error)
    }

    /// Drop the current notice if it has been up long enough.
    ///
    /// Returns whether anything changed, so the caller knows to repaint.
    pub fn expire_notice(&mut self) -> bool {
        let expired = self
            .notice
            .as_ref()
            .is_some_and(|n| n.is_expired(self.tick));
        if expired {
            self.dismiss_notice();
        }
        expired
    }

    /// Number of overlays currently open. Should only ever be 0 or 1.
    #[cfg(test)]
    pub fn open_overlay_count(&self) -> usize {
        [
            self.diagram.is_some(),
            self.notice_detail_open,
            self.mutation.cell_edit.is_some(),
            self.filter.visible,
            self.conn.form.visible,
            self.conn.pending_delete.is_some(),
            self.theme_picker.visible,
        ]
        .iter()
        .filter(|open| **open)
        .count()
    }

    pub fn new(connections: Vec<ConnectionConfig>) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_placeholder_text("-- Write SQL here. Press Ctrl+S or F5 to run.");

        Self {
            focused: FocusedPanel::Connections,

            conn: ConnectionState {
                connections,
                discovered: Vec::new(),
                cursor: ListCursor::new(),
                active_id: None,
                active_backend: DbBackend::Postgres,
                form: ConnectionForm::default(),
                pending_delete: None,
            },
            tables: TableBrowserState {
                tables: Vec::new(),
                cursor: ListCursor::new(),
            },
            editor: EditorState {
                textarea,
                mode: EditorMode::Normal,
                scroll_row: 0,
                scroll_col: 0,
                revision: 0,
                highlighter: SqlHighlighter::new(),
                completion: CompletionState::default(),
                dragging: false,
            },
            results: ResultsState {
                data: QueryResult::default(),
                source_sql: None,
                sent_sql: None,
                awaiting_new_query: false,
                scroll: 0,
                col_scroll: 0,
                selected_row: 0,
                selected_col: 0,
                current_page: 0,
                is_loading: false,
                sort: None,
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
                results_area: None,
                last_col_widths: Vec::new(),
                spinner_frame: 0,
                sidebar_hidden: false,
                needs_redraw: true,
                hits: HitMap::default(),
                editor_text_rect: None,
                last_drag: None,
                last_click: None,
            },

            filter: FilterBar::default(),
            active_filter: None,
            diagram: None,
            diagram_requested: false,
            cached_diagram: None,
            notice: None,
            notice_detail_open: false,
            theme_picker: ThemePicker::default(),
            tick: 0,
            should_quit: false,
        }
    }

    /// Resolve every primary key component of a displayed row to its value.
    ///
    /// Returns `None` when there is no primary key or any component cannot be
    /// found in the result set — using a *partial* key instead would make the
    /// eventual UPDATE/DELETE match more rows than the one the user selected.
    fn resolve_row_pk(
        &self,
        row_idx: usize,
        pk_columns: &[String],
    ) -> Option<Vec<(String, String)>> {
        if pk_columns.is_empty() {
            return None;
        }
        let row = self.results.data.rows.get(row_idx)?;
        pk_columns
            .iter()
            .map(|pk_col| {
                self.results
                    .data
                    .columns
                    .iter()
                    .position(|c| c.to_lowercase() == pk_col.to_lowercase())
                    .and_then(|ci| row.get(ci))
                    .filter(|val| !val.is_empty())
                    .map(|val| (pk_col.clone(), val.clone()))
            })
            .collect()
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
                self.conn.cursor.clamp(self.conn.len());
            }
            CoreEvent::DiscoveredConnections(found) => {
                // Startup discovery, so it must not touch `is_loading`: the
                // user may already be waiting on a query they asked for.
                let count = found.len();
                self.conn.discovered = found;
                self.conn.cursor.clamp(self.conn.len());
                // Only worth saying when there is something to say. Silence is
                // the right answer on a machine with no Docker.
                if count > 0 && self.notice.is_none() {
                    self.inform(format!(
                        "Found {count} database(s) running in Docker — listed below your saved ones."
                    ));
                }
            }
            CoreEvent::Connected(id) => {
                self.results.is_loading = false;
                self.conn.active_id = Some(id);
                // A discovered connection is connectable too, so both lists are
                // searched — otherwise the backend and the name would be wrong
                // for exactly the connections Docker found for us.
                let found = self
                    .conn
                    .entries()
                    .map(|e| e.config())
                    .find(|c| c.id == id)
                    .map(|c| (c.backend, c.name.clone()));
                if let Some(backend) = found.as_ref().map(|(b, _)| *b) {
                    self.conn.active_backend = backend;
                }
                let name = found.map_or_else(|| id.to_string(), |(_, name)| name);
                self.inform(format!("Connected to {name}"));
            }
            CoreEvent::Disconnected(id) => {
                self.results.is_loading = false;
                if self.conn.active_id == Some(id) {
                    self.conn.active_id = None;
                }
                self.tables.tables.clear();
                self.inform("Disconnected");
            }
            CoreEvent::TableList(tables) => {
                self.results.is_loading = false;
                self.tables.tables = tables;
                self.tables.cursor.reset();
            }
            CoreEvent::QueryResult(mut result) => {
                self.results.is_loading = false;
                // Which way the page moved decides where the cursor lands, so
                // it has to be read before `current_page` is overwritten.
                let previous_page = self.results.current_page;
                self.results.current_page = result.page;
                tracing::info!(
                    "QueryResult: page={} rows={} cols={} has_next={}",
                    result.page,
                    result.rows.len(),
                    result.columns.len(),
                    result.has_next_page,
                );
                // Reading forward has to continue where the last page left
                // off. Keeping the cursor's index across a page boundary put it
                // at row 99 of the *next* page — eighty rows of content skipped
                // in one keypress, and the very next press paged again, so
                // scrolling through a large table saw one row in every hundred.
                let fresh = std::mem::take(&mut self.results.awaiting_new_query);
                let landing = if fresh || result.page > previous_page {
                    // A fresh query and the next page while reading forward
                    // both start at the first row.
                    Landing::Top
                } else if result.page < previous_page {
                    // Paging back: land on the row adjacent to where you were,
                    // which going up means the last one.
                    Landing::Bottom
                } else {
                    // The same page again — a refresh after a commit. Moving
                    // the cursor would lose the row the user was working on.
                    Landing::Keep
                };
                if landing == Landing::Top {
                    self.results.col_scroll = 0;
                    self.results.selected_col = 0;
                }
                // These rows came from the SQL last sent for execution; record
                // it so edits/deletes target the table that actually produced
                // them, not whatever the editor says now.
                if let Some(sql) = self.results.sent_sql.clone() {
                    self.results.source_sql = Some(sql);
                }
                // A new result set invalidates staged changes — but never
                // silently. Paging is blocked while changes are staged, so
                // reaching this with staged work means a new query ran.
                let discarded =
                    self.mutation.pending_edits.len() + self.mutation.pending_deletes.len();
                self.mutation.discard_pending();

                // Preserve previous columns when current page has no rows.
                if result.columns.is_empty() && !self.results.data.columns.is_empty() {
                    result.columns = self.results.data.columns.clone();
                }

                self.results.data = result;
                // On page > 0 the selection is not reset, so a shorter page
                // (typically the last one) could leave selected_row past the
                // end — every later row op would then read out of bounds or act
                // on the wrong row. Clamp it to the new row count.
                let row_count = self.results.data.rows.len();
                match landing {
                    Landing::Top => {
                        self.results.selected_row = 0;
                        self.results.scroll = 0;
                    }
                    Landing::Bottom => {
                        self.results.selected_row = row_count.saturating_sub(1);
                        self.results.scroll =
                            row_count.saturating_sub(self.results.viewport_height.max(1));
                    }
                    // A shorter page — typically the last one — can leave the
                    // kept index past the end, and every later row op would
                    // then act on the wrong row or read out of bounds.
                    Landing::Keep => {
                        if row_count == 0 {
                            self.results.selected_row = 0;
                        } else if self.results.selected_row >= row_count {
                            self.results.selected_row = row_count - 1;
                        }
                    }
                }
                let col_count = self.results.data.columns.len();
                if col_count == 0 {
                    self.results.selected_col = 0;
                } else if self.results.selected_col >= col_count {
                    self.results.selected_col = col_count - 1;
                }
                self.results.col_widths_dirty = true;
                self.dismiss_notice();
                if discarded > 0 {
                    self.report(format!(
                        "{discarded} staged change(s) discarded — a new result set arrived."
                    ));
                }
            }
            // The only place the sort cache is written. Deliberately does not
            // touch `is_loading`: this rides along with whatever command
            // changed the sort, and clearing the spinner here would hide the
            // query that is still running.
            CoreEvent::SortChanged(sort) => {
                self.results.sort = sort;
            }
            // The TUI derives its "rows 1–N" range from the page it is showing,
            // so it never asks for a count and has nothing to do with one.
            CoreEvent::TotalCount(_) => {}
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

                // Resolve a pending delete if one is waiting for this PK info.
                if let Some(row_idx) = self.mutation.pending_delete_row.take() {
                    match self.resolve_row_pk(row_idx, &columns) {
                        Some(pk) => {
                            // Toggle: if already marked, unmark
                            if let std::collections::hash_map::Entry::Vacant(e) =
                                self.mutation.pending_deletes.entry(row_idx)
                            {
                                e.insert(PendingDelete { schema, table, pk });
                            } else {
                                self.mutation.pending_deletes.remove(&row_idx);
                            }
                        }
                        None => {
                            self.report("Cannot mark for delete: primary key not found.");
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

                    let Some(pk) = self.resolve_row_pk(row_idx, &columns) else {
                        self.report("Cannot edit: primary key value not found in result set.");
                        return;
                    };

                    self.mutation.cell_edit = Some(CellEditState::new(
                        row_idx, col_idx, col_name, original, schema, table, pk,
                    ));
                }
            }
            CoreEvent::Loading => {
                self.results.is_loading = true;
                // A failure from the previous attempt must not sit next to a
                // spinner for the next one.
                self.dismiss_notice();
            }
            CoreEvent::DiagramLoaded(data) => {
                self.results.is_loading = false;
                self.cached_diagram = Some(data.clone());
                if self.diagram_requested {
                    self.diagram_requested = false;
                    self.close_overlays();
                    self.diagram = Some(DiagramState::new(data));
                }
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
                    self.filter
                        .suggestion_cursor
                        .clamp(self.filter.suggestions.len());
                    self.filter.loading_suggestions = false;
                }
            }
            CoreEvent::Error(err) => {
                self.results.is_loading = false;
                self.report_core(err);
                self.mutation.pending_cell_edit = None;
                self.mutation.pending_delete_row = None;
                self.filter.loading_suggestions = false;
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
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
            total_count: None,
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
        state.results.move_row_up_with_page_hint();
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

    /// The `o` cycle, read off the cache core keeps up to date.
    #[test]
    fn test_app_state_sort_toggle() {
        let mut state = AppState::new(vec![]);

        assert_eq!(
            state.results.next_sort_direction("id"),
            Some(SortDirection::Ascending)
        );

        state.apply_core_event(CoreEvent::SortChanged(Some((
            "id".into(),
            SortDirection::Ascending,
        ))));
        assert_eq!(
            state.results.next_sort_direction("id"),
            Some(SortDirection::Descending)
        );

        state.apply_core_event(CoreEvent::SortChanged(Some((
            "id".into(),
            SortDirection::Descending,
        ))));
        assert_eq!(state.results.next_sort_direction("id"), None);

        state.apply_core_event(CoreEvent::SortChanged(None));
        assert_eq!(
            state.results.next_sort_direction("id"),
            Some(SortDirection::Ascending)
        );
    }

    /// Sorting a second column starts that column's cycle at ascending rather
    /// than inheriting the first column's position in it.
    #[test]
    fn sorting_a_different_column_restarts_the_cycle() {
        let mut state = AppState::new(vec![]);
        state.apply_core_event(CoreEvent::SortChanged(Some((
            "id".into(),
            SortDirection::Descending,
        ))));

        assert_eq!(
            state.results.next_sort_direction("name"),
            Some(SortDirection::Ascending)
        );
    }

    /// Core drops the sort on its own when a connection closes. Before the
    /// cache was fed solely from `SortChanged`, the TUI never heard about it
    /// and went on drawing an arrow for an ORDER BY that was gone.
    #[test]
    fn a_dropped_sort_leaves_nothing_behind() {
        let mut state = AppState::new(vec![]);
        state.apply_core_event(CoreEvent::SortChanged(Some((
            "name".into(),
            SortDirection::Ascending,
        ))));

        state.apply_core_event(CoreEvent::Disconnected(uuid::Uuid::new_v4()));
        state.apply_core_event(CoreEvent::SortChanged(None));

        assert!(state.results.sort.is_none());
    }

    /// A count arriving from core is a no-op here — the TUI shows "rows 1–N"
    /// from the page it has — but it must not disturb anything either.
    #[test]
    fn a_total_count_changes_nothing() {
        let mut state = AppState::new(vec![]);
        state.results.is_loading = true;

        state.apply_core_event(CoreEvent::TotalCount(Some(1_000)));

        assert!(state.results.is_loading);
    }

    // -----------------------------------------------------------------------
    // apply_core_event tests
    // -----------------------------------------------------------------------

    #[test]
    fn core_event_connection_list() {
        let mut state = AppState::new(vec![]);
        let conns = vec![
            ConnectionConfig::new_postgres("a", "h", 5432, "u", "d"),
            ConnectionConfig::new_postgres("b", "h", 5432, "u", "d"),
        ];
        state.apply_core_event(CoreEvent::ConnectionList(conns));
        assert_eq!(state.conn.connections.len(), 2);
    }

    /// Build a discovery the way the core would report one.
    fn discovered(name: &str) -> DiscoveredConnection {
        DiscoveredConnection {
            config: ConnectionConfig::new_postgres(name, "127.0.0.1", 5432, "u", "d"),
            source: sbql_core::DiscoverySource::Container { name: name.into() },
        }
    }

    /// Two lists, one cursor. Indexing only the saved ones would leave the
    /// discovered rows drawn but unreachable.
    #[test]
    fn the_cursor_walks_saved_and_discovered_as_one_list() {
        let mut state = AppState::new(vec![ConnectionConfig::new_postgres(
            "saved", "h", 5432, "u", "d",
        )]);
        state.conn.discovered = vec![discovered("from-docker")];

        assert_eq!(state.conn.len(), 2);

        state.conn.cursor.select(1, state.conn.len());
        let entry = state.conn.selected_entry().expect("a row under the cursor");
        assert!(entry.is_discovered());
        assert_eq!(entry.config().name, "from-docker");

        state.conn.cursor.select(0, state.conn.len());
        assert!(!state.conn.selected_entry().expect("a row").is_discovered());
    }

    /// A discovery arriving must not disturb a query the user is waiting on,
    /// and must not be mistaken for the saved list.
    #[test]
    fn core_event_discovered_connections_leaves_saved_ones_alone() {
        let mut state = AppState::new(vec![ConnectionConfig::new_postgres(
            "saved", "h", 5432, "u", "d",
        )]);
        state.results.is_loading = true;

        state.apply_core_event(CoreEvent::DiscoveredConnections(vec![discovered("pg")]));

        assert_eq!(state.conn.connections.len(), 1, "saved list untouched");
        assert_eq!(state.conn.discovered.len(), 1);
        assert!(
            state.results.is_loading,
            "a startup scan must not clear a spinner the user is waiting on"
        );
    }

    #[test]
    fn core_event_connection_list_clamps_selected() {
        let mut state = AppState::new(vec![
            ConnectionConfig::new_postgres("a", "h", 5432, "u", "d"),
            ConnectionConfig::new_postgres("b", "h", 5432, "u", "d"),
        ]);
        state.conn.cursor.select(1, state.conn.connections.len());
        // Now replace with just 1 connection
        state.apply_core_event(CoreEvent::ConnectionList(vec![
            ConnectionConfig::new_postgres("a", "h", 5432, "u", "d"),
        ]));
        assert_eq!(state.conn.selected(), 0);
    }

    #[test]
    fn core_event_connected() {
        let mut state = AppState::new(vec![]);
        let id = Uuid::new_v4();
        state.apply_core_event(CoreEvent::Connected(id));
        assert_eq!(state.conn.active_id, Some(id));
        assert!(
            state
                .notice_text()
                .is_some_and(|t| t.starts_with("Connected")),
            "{:?}",
            state.notice_text()
        );
        assert!(!state.is_failing());
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
        state.tables.cursor.select(5, 6);
        let tables = vec![
            TableEntry {
                schema: "public".into(),
                name: "users".into(),
            },
            TableEntry {
                schema: "public".into(),
                name: "posts".into(),
            },
        ];
        state.apply_core_event(CoreEvent::TableList(tables));
        assert_eq!(state.tables.tables.len(), 2);
        assert_eq!(state.tables.selected(), 0); // reset
    }

    #[test]
    fn core_event_query_result_page_0_resets() {
        let mut state = AppState::new(vec![]);
        // Page 0 on its own is ambiguous — it is also what a refresh after a
        // commit returns — so what makes this a reset is that a query was
        // sent, which is the flag the editor sets.
        state.results.awaiting_new_query = true;
        state.results.selected_row = 5;
        state.results.selected_col = 3;
        state.results.scroll = 10;
        let result = QueryResult {
            columns: vec!["id".into()],
            rows: vec![vec!["1".into()]],
            page: 0,
            has_next_page: false,
            total_count: None,
        };
        state.apply_core_event(CoreEvent::QueryResult(result));
        assert_eq!(state.results.selected_row, 0);
        assert_eq!(state.results.selected_col, 0);
        assert_eq!(state.results.scroll, 0);
        assert_eq!(state.results.current_page, 0);
    }

    #[test]
    fn core_event_query_result_forward_page_starts_at_the_top() {
        let mut state = AppState::new(vec![]);
        state.results.selected_row = 5;
        state.results.selected_col = 0;
        // This test used to assert the opposite — that row 5 survived the page
        // change — which is the bug: row 5 of the next page is different data,
        // and everything between was skipped.
        let result = QueryResult {
            columns: vec!["id".into()],
            rows: (0..10).map(|i| vec![i.to_string()]).collect(),
            page: 2,
            has_next_page: false,
            total_count: None,
        };
        state.apply_core_event(CoreEvent::QueryResult(result));
        assert_eq!(state.results.selected_row, 0);
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
            total_count: None,
        };
        state.apply_core_event(CoreEvent::QueryResult(result));
        assert_eq!(
            state.results.data.columns,
            vec!["id".to_string(), "name".to_string()]
        );
    }

    #[test]
    fn core_event_cell_updated() {
        let mut state = AppState::new(vec![]);
        state.mutation.cell_edit = Some(CellEditState::new(
            0,
            0,
            "id".into(),
            "1".into(),
            "public".into(),
            "t".into(),
            vec![("id".into(), "1".into())],
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
        assert!(state.notice.is_none());
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
        assert_eq!(ce.pk, vec![("id".to_string(), "1".to_string())]);
    }

    /// A composite key must keep every component — reduced to its first
    /// column, "delete this order line" became "delete the whole order".
    #[test]
    fn core_event_primary_keys_composite_delete_keeps_all_components() {
        let mut state = AppState::new(vec![]);
        state.results.data.columns = vec!["order_id".into(), "line_no".into(), "qty".into()];
        state.results.data.rows = vec![
            vec!["42".into(), "1".into(), "3".into()],
            vec!["42".into(), "2".into(), "5".into()],
        ];
        state.mutation.pending_delete_row = Some(0);
        state.apply_core_event(CoreEvent::PrimaryKeys {
            schema: "public".into(),
            table: "order_lines".into(),
            columns: vec!["order_id".into(), "line_no".into()],
        });
        let del = state
            .mutation
            .pending_deletes
            .get(&0)
            .expect("row 0 should be marked");
        assert_eq!(
            del.pk,
            vec![
                ("order_id".to_string(), "42".to_string()),
                ("line_no".to_string(), "1".to_string()),
            ]
        );
    }

    /// If any PK component cannot be resolved from the result set, refuse:
    /// a partial key would match more rows than the one the user picked.
    #[test]
    fn core_event_primary_keys_missing_component_refuses() {
        let mut state = AppState::new(vec![]);
        // The query didn't select line_no, so the key cannot be completed.
        state.results.data.columns = vec!["order_id".into(), "qty".into()];
        state.results.data.rows = vec![vec!["42".into(), "3".into()]];
        state.mutation.pending_delete_row = Some(0);
        state.apply_core_event(CoreEvent::PrimaryKeys {
            schema: "public".into(),
            table: "order_lines".into(),
            columns: vec!["order_id".into(), "line_no".into()],
        });
        assert!(state.mutation.pending_deletes.is_empty());
        assert!(state.is_failing());
    }

    /// A page of `n` rows, as the core would deliver it.
    fn page_of(n: usize, page: usize, has_next: bool) -> QueryResult {
        QueryResult {
            columns: vec!["id".into()],
            rows: (0..n).map(|i| vec![i.to_string()]).collect(),
            page,
            has_next_page: has_next,
            total_count: None,
        }
    }

    /// The bug this guards: pressing down at the bottom of a page fetched the
    /// next one and kept the cursor's index, so the user landed on row 99 of
    /// the new page — eighty rows skipped, and the next press paged again.
    #[test]
    fn crossing_into_the_next_page_lands_on_its_first_row() {
        let mut state = AppState::new(vec![]);
        state.results.viewport_height = 20;
        state.apply_core_event(CoreEvent::QueryResult(page_of(100, 0, true)));

        // Walk to the bottom of page 0 the way the keyboard does.
        for _ in 0..99 {
            assert!(
                !state.results.move_row_down_with_page_hint(),
                "no page should be requested before the last row"
            );
        }
        assert_eq!(state.results.selected_row, 99);
        assert!(
            state.results.move_row_down_with_page_hint(),
            "the last row asks for the next page"
        );

        state.apply_core_event(CoreEvent::QueryResult(page_of(100, 1, true)));

        assert_eq!(
            state.results.selected_row, 0,
            "reading continues at the top"
        );
        assert_eq!(state.results.scroll, 0, "and the view follows it");
    }

    /// Going back should put you next to where you were, which upwards means
    /// the last row of the previous page.
    #[test]
    fn paging_backwards_lands_on_the_last_row() {
        let mut state = AppState::new(vec![]);
        state.results.viewport_height = 20;
        state.apply_core_event(CoreEvent::QueryResult(page_of(100, 2, true)));

        state.apply_core_event(CoreEvent::QueryResult(page_of(100, 1, true)));

        assert_eq!(state.results.selected_row, 99);
        assert_eq!(state.results.scroll, 80, "the last row is in view");
    }

    /// The same page arriving again is a refresh after a commit, not a move.
    #[test]
    fn refetching_the_same_page_keeps_the_cursor() {
        let mut state = AppState::new(vec![]);
        state.results.viewport_height = 20;
        state.apply_core_event(CoreEvent::QueryResult(page_of(100, 3, true)));
        state.results.selected_row = 42;
        state.results.scroll = 30;

        state.apply_core_event(CoreEvent::QueryResult(page_of(100, 3, true)));

        assert_eq!(state.results.selected_row, 42);
        assert_eq!(state.results.scroll, 30);
    }

    /// Scrolling a long table has to move one row per press, with the view
    /// following only when the cursor would otherwise leave it. A jump of more
    /// than one row is the thing being ruled out.
    #[test]
    fn scrolling_a_long_table_moves_exactly_one_row_at_a_time() {
        let mut state = AppState::new(vec![]);
        state.results.viewport_height = 20;
        state.apply_core_event(CoreEvent::QueryResult(page_of(500, 0, false)));

        for step in 0..499 {
            let (before_row, before_scroll) = (state.results.selected_row, state.results.scroll);
            state.results.move_row_down_with_page_hint();

            assert_eq!(
                state.results.selected_row,
                before_row + 1,
                "step {step} moved more than one row"
            );
            let scrolled = state.results.scroll - before_scroll;
            assert!(scrolled <= 1, "step {step} scrolled the view by {scrolled}");
            // The cursor must stay on screen, or the user is editing a row
            // they cannot see.
            assert!(
                state.results.selected_row >= state.results.scroll
                    && state.results.selected_row < state.results.scroll + 20,
                "step {step} left the cursor off screen"
            );
        }
        assert_eq!(state.results.selected_row, 499);
        assert_eq!(state.results.scroll, 480, "the last screenful is in view");
    }

    /// Reading a long table end to end must see every row, not one per page.
    #[test]
    fn reading_forward_across_pages_visits_every_row() {
        let mut state = AppState::new(vec![]);
        state.results.viewport_height = 20;
        state.apply_core_event(CoreEvent::QueryResult(page_of(100, 0, true)));

        let mut seen = 0usize;
        for page in 0..3 {
            assert_eq!(
                state.results.selected_row, 0,
                "page {page} has to start at its first row"
            );
            // Every row of this page, top to bottom. Counting keypresses would
            // not do: with the cursor stuck at the bottom of a page, the presses
            // still happen while nothing moves.
            for row in 0..99 {
                state.results.move_row_down_with_page_hint();
                assert_eq!(
                    state.results.selected_row,
                    row + 1,
                    "page {page} stopped advancing at row {row}"
                );
                seen += 1;
            }
            assert_eq!(
                state.results.selected_row, 99,
                "page {page} ends at its last row"
            );
            assert!(
                state.results.move_row_down_with_page_hint(),
                "the last row of page {page} asks for the next"
            );
            seen += 1;
            state.apply_core_event(CoreEvent::QueryResult(page_of(100, page + 1, page < 2)));
        }
        assert_eq!(seen, 300, "every row of all three pages was visited");
    }

    /// The other half of reading a large table: having crossed into a page,
    /// the rows above have to remain reachable. Landing on the first row was a
    /// dead end — up did nothing, and the only way back was to close the table
    /// and open it again.
    #[test]
    fn the_first_row_of_a_page_asks_for_the_previous_one() {
        let mut state = AppState::new(vec![]);
        state.results.viewport_height = 20;
        state.apply_core_event(CoreEvent::QueryResult(page_of(100, 1, true)));

        assert_eq!(
            state.results.selected_row, 0,
            "forward paging lands at the top"
        );
        assert!(
            state.results.move_row_up_with_page_hint(),
            "up from the first row has to ask for the page before it"
        );
    }

    /// On the very first page there is nothing above, so up is simply the end
    /// of the road rather than a request that can never be answered.
    #[test]
    fn the_first_row_of_the_first_page_asks_for_nothing() {
        let mut state = AppState::new(vec![]);
        state.results.viewport_height = 20;
        state.results.awaiting_new_query = true;
        state.apply_core_event(CoreEvent::QueryResult(page_of(100, 0, true)));

        assert!(!state.results.move_row_up_with_page_hint());
        assert_eq!(state.results.selected_row, 0);
    }

    /// Paging back has to put the cursor on the last row, or the rows just
    /// above the boundary are skipped on the way up exactly as they were on
    /// the way down.
    #[test]
    fn reading_backwards_across_pages_visits_every_row() {
        let mut state = AppState::new(vec![]);
        state.results.viewport_height = 20;
        // Start where forward reading would have left us: the top of page 2.
        state.apply_core_event(CoreEvent::QueryResult(page_of(100, 2, false)));

        for page in (0..2).rev() {
            assert!(
                state.results.move_row_up_with_page_hint(),
                "the top of a page asks for the one before it"
            );
            state.apply_core_event(CoreEvent::QueryResult(page_of(100, page, true)));
            assert_eq!(
                state.results.selected_row, 99,
                "page {page} is entered at its last row"
            );

            // And every row of it is walked, one at a time.
            for row in (0..99).rev() {
                state.results.move_row_up_with_page_hint();
                assert_eq!(
                    state.results.selected_row, row,
                    "page {page} stopped moving at row {row}"
                );
            }
        }
        assert_eq!(state.results.current_page, 0);
    }

    /// Down then straight back up returns to the row you left, rather than to
    /// some other page's row with the same index.
    #[test]
    fn crossing_a_boundary_and_turning_back_returns_where_you_were() {
        let mut state = AppState::new(vec![]);
        state.results.viewport_height = 20;
        state.results.awaiting_new_query = true;
        state.apply_core_event(CoreEvent::QueryResult(page_of(100, 0, true)));
        for _ in 0..99 {
            state.results.move_row_down_with_page_hint();
        }

        // Over the edge into page 1...
        assert!(state.results.move_row_down_with_page_hint());
        state.apply_core_event(CoreEvent::QueryResult(page_of(100, 1, true)));
        assert_eq!(state.results.selected_row, 0);

        // ...and straight back.
        assert!(state.results.move_row_up_with_page_hint());
        state.apply_core_event(CoreEvent::QueryResult(page_of(100, 0, true)));

        assert_eq!(state.results.current_page, 0);
        assert_eq!(state.results.selected_row, 99, "back on the row we left");
    }

    /// A shorter later page must not leave the selection past the last row.
    #[test]
    fn core_event_query_result_clamps_selection_on_short_page() {
        let mut state = AppState::new(vec![]);
        // The same page arriving again, so the cursor is kept rather than
        // moved — which is the only case where clamping has anything to do.
        // A shrinking refresh is real: rows can be deleted under you.
        state.results.current_page = 3;
        state.results.selected_row = 40;
        state.results.selected_col = 5;
        state.apply_core_event(CoreEvent::QueryResult(QueryResult {
            columns: vec!["id".into(), "name".into()],
            rows: vec![vec!["1".into(), "a".into()], vec!["2".into(), "b".into()]],
            page: 3,
            has_next_page: false,
            total_count: None,
        }));
        assert_eq!(state.results.selected_row, 1, "clamped to last row");
        assert_eq!(state.results.selected_col, 1, "clamped to last column");
    }

    /// The result set records the SQL that actually produced it.
    #[test]
    fn core_event_query_result_promotes_sent_sql() {
        let mut state = AppState::new(vec![]);
        state.results.sent_sql = Some("SELECT * FROM orders".into());
        state.apply_core_event(CoreEvent::QueryResult(QueryResult {
            columns: vec!["id".into()],
            rows: vec![vec!["1".into()]],
            page: 0,
            has_next_page: false,
            total_count: None,
        }));
        assert_eq!(
            state.results.source_sql.as_deref(),
            Some("SELECT * FROM orders")
        );
    }

    /// Staged work must never disappear without a word.
    #[test]
    fn core_event_query_result_reports_discarded_staged_changes() {
        let mut state = AppState::new(vec![]);
        state.mutation.pending_deletes.insert(
            0,
            PendingDelete {
                schema: "public".into(),
                table: "users".into(),
                pk: vec![("id".into(), "1".into())],
            },
        );
        state.apply_core_event(CoreEvent::QueryResult(QueryResult {
            columns: vec!["id".into()],
            rows: vec![],
            page: 0,
            has_next_page: false,
            total_count: None,
        }));
        assert!(state.mutation.pending_deletes.is_empty());
        assert!(
            state.notice_text().is_some_and(|t| t.contains("discarded")),
            "{:?}",
            state.notice_text()
        );
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
        assert!(state.is_failing());
    }

    #[test]
    fn core_event_diagram_loaded_without_request() {
        let mut state = AppState::new(vec![]);
        let data = DiagramData::default();
        state.apply_core_event(CoreEvent::DiagramLoaded(data));
        // Should NOT open overlay unless user requested it
        assert!(state.diagram.is_none());
        assert!(state.cached_diagram.is_some());
    }

    #[test]
    fn core_event_diagram_loaded_with_request() {
        let mut state = AppState::new(vec![]);
        state.diagram_requested = true;
        let data = DiagramData::default();
        state.apply_core_event(CoreEvent::DiagramLoaded(data));
        assert!(state.diagram.is_some());
        assert!(state.cached_diagram.is_some());
        assert!(!state.diagram_requested);
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

    /// A core failure reaches the bar with everything the core knew about it,
    /// not just its text.
    #[test]
    fn core_event_error() {
        let mut state = AppState::new(vec![]);
        state.results.is_loading = true;
        state.apply_core_event(CoreEvent::Error(
            sbql_core::CoreError::new(sbql_core::ErrorKind::Query, "something failed")
                .with_detail("near line 1"),
        ));

        assert!(!state.results.is_loading);
        assert!(state.is_failing());
        let notice = state.notice.as_ref().expect("a notice");
        assert_eq!(notice.text, "something failed");
        assert_eq!(notice.detail.as_deref(), Some("near line 1"));
        assert_eq!(notice.kind, Some(sbql_core::ErrorKind::Query));
    }

    /// The severity survives the trip, so a caveat is not painted as a failure.
    #[test]
    fn a_core_warning_stays_a_warning() {
        let mut state = AppState::new(vec![]);
        state.apply_core_event(CoreEvent::Error(sbql_core::CoreError::warning(
            sbql_core::ErrorKind::Credentials,
            "saved, password not stored",
        )));

        assert!(!state.is_failing(), "a warning is not a failure");
        assert_eq!(
            state.notice.as_ref().map(|n| n.level),
            Some(crate::notice::Level::Warning)
        );
    }

    /// The bug two fields made possible: a confirmation arriving after a
    /// failure and never being seen.
    #[test]
    fn a_later_message_replaces_an_earlier_failure() {
        let mut state = AppState::new(vec![]);
        state.apply_core_event(CoreEvent::Error(sbql_core::CoreError::new(
            sbql_core::ErrorKind::Connection,
            "connection refused",
        )));
        assert!(state.is_failing());

        let id = uuid::Uuid::new_v4();
        state.apply_core_event(CoreEvent::Disconnected(id));

        assert_eq!(state.notice_text(), Some("Disconnected"));
        assert!(!state.is_failing(), "the old failure should be gone");
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

    /// Labels follow the backend rather than the storage field, so switching
    /// backend must relabel the rows without any per-backend code here.
    #[test]
    fn connection_form_labels_follow_the_selected_backend() {
        let mut form = ConnectionForm::default();
        form.draft.set_backend(DbBackend::DynamoDb);
        assert_eq!(form.field_label(2), "Endpoint");
        assert_eq!(form.field_label(4), "Region");

        form.draft.set_backend(DbBackend::Sqlite);
        assert_eq!(form.field_label(2), "File Path");
        assert_eq!(form.field_count(), 3);
    }

    /// A validation error has to point at a row, so the cursor can land on the
    /// field the user still needs to fill in.
    #[test]
    fn connection_form_maps_a_field_back_to_its_row() {
        let form = ConnectionForm::default(); // Postgres
        assert_eq!(form.row_of(sbql_core::ConnectionField::Name), Some(1));
        assert_eq!(form.row_of(sbql_core::ConnectionField::Database), Some(5));
        // Postgres has no file path, so there is no row to point at.
        assert_eq!(form.row_of(sbql_core::ConnectionField::FilePath), None);
    }

    #[test]
    fn connection_form_redis_field_labels() {
        let mut form = ConnectionForm::default();
        form.draft.backend = DbBackend::Redis;
        assert_eq!(form.field_label(0), "Backend");
        assert_eq!(form.field_label(1), "Name");
        assert_eq!(form.field_label(2), "Host");
        assert_eq!(form.field_label(3), "Port");
        assert_eq!(form.field_label(4), "Password");
        assert_eq!(form.field_label(5), "Database");
        assert_eq!(form.field_label(6), "");
    }

    #[test]
    fn connection_form_redis_field_count() {
        let mut form = ConnectionForm::default();
        form.draft.backend = DbBackend::Redis;
        assert_eq!(form.field_count(), 6);
    }

    #[test]
    fn connection_form_cycle_backend_visits_every_backend() {
        let mut form = ConnectionForm::default();
        assert_eq!(form.draft.backend, DbBackend::Postgres);

        let expected = [
            DbBackend::Mysql,
            DbBackend::Sqlite,
            DbBackend::Redis,
            DbBackend::DynamoDb,
            DbBackend::MongoDb,
            DbBackend::SqlServer,
            DbBackend::Postgres,
        ];
        for backend in expected {
            form.cycle_backend();
            assert_eq!(form.draft.backend, backend);
        }
    }

    #[test]
    fn connection_form_redis_active_value_mut() {
        let mut form = ConnectionForm::default();
        form.draft.backend = DbBackend::Redis;

        form.field_index = 0;
        assert!(form.active_value_mut().is_none()); // Backend is cycled

        form.field_index = 1;
        *form.active_value_mut().unwrap() = "my-redis".into();
        assert_eq!(form.draft.name, "my-redis");

        form.field_index = 2;
        *form.active_value_mut().unwrap() = "redis-host".into();
        assert_eq!(form.draft.host, "redis-host");

        form.field_index = 3;
        *form.active_value_mut().unwrap() = "6379".into();
        assert_eq!(form.draft.port, "6379");

        form.field_index = 4;
        *form.active_value_mut().unwrap() = "secret".into();
        assert_eq!(form.draft.password, "secret");

        form.field_index = 5;
        *form.active_value_mut().unwrap() = "2".into();
        assert_eq!(form.draft.database, "2");
    }

    #[test]
    fn connection_form_cycle_ssl() {
        let mut form = ConnectionForm::default();
        form.cycle_ssl_mode();
        assert_eq!(form.draft.ssl_mode, SslMode::Require);
        form.cycle_ssl_mode();
        assert_eq!(form.draft.ssl_mode, SslMode::VerifyFull);
        form.cycle_ssl_mode();
        assert_eq!(form.draft.ssl_mode, SslMode::VerifyCa);
        form.cycle_ssl_mode();
        assert_eq!(form.draft.ssl_mode, SslMode::Disable);
        form.cycle_ssl_mode();
        assert_eq!(form.draft.ssl_mode, SslMode::Prefer);
    }

    #[test]
    fn connection_form_active_value_mut() {
        let mut form = ConnectionForm::default();
        form.field_index = 0;
        assert!(form.active_value_mut().is_none()); // Backend is cycled

        form.field_index = 1;
        *form.active_value_mut().unwrap() = "test".into();
        assert_eq!(form.draft.name, "test");

        form.field_index = 7;
        assert!(form.active_value_mut().is_none()); // SSL mode is cycled
    }

    #[test]
    fn connection_form_open_new() {
        let form = ConnectionForm::open_new();
        assert!(form.visible);
        assert_eq!(form.draft.port, "5432");
        assert_eq!(form.draft.host, "localhost");
    }

    #[test]
    fn connection_form_open_edit() {
        let cfg = ConnectionConfig::new_postgres("myconn", "myhost", 3333, "myuser", "mydb");
        let form = ConnectionForm::open_edit(&cfg);
        assert!(form.visible);
        assert_eq!(form.draft.name, "myconn");
        assert_eq!(form.draft.host, "myhost");
        assert_eq!(form.draft.port, "3333");
        assert!(form.draft.id.is_some());
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
        ms.pending_edits.insert(
            (0, 0),
            PendingEdit {
                new_val: "x".into(),
                schema: "p".into(),
                table: "t".into(),
                pk: vec![("id".into(), "1".into())],
                col_name: "c".into(),
            },
        );
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
        state.results.move_row_up_with_page_hint();
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
