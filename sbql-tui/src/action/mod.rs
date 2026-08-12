mod cell_edit;
mod completion_ui;
mod connections;
mod diagram;
mod editor;
mod filter;
mod form;
mod nav;
mod results;
mod tables;

use tokio::sync::mpsc;
use tui_textarea::{CursorMove, Input};

use crate::app::{
    AppState, ConnectionForm, DiagramGlyphMode, EditorMode, FocusedPanel, NavMode, PendingEdit,
};
use crate::completion;
use crate::list_cursor::Overflow;
use sbql_core::CoreCommand;

/// A pure description of a state change or side effect.
///
/// Handlers produce `Action` values without mutating state directly.
/// The event loop calls [`apply`] to execute them.
#[derive(Debug)]
pub enum Action {
    /// Focus, modes, and pending vim prefixes.
    Nav(NavAction),
    /// Moving around and acting on the results grid.
    Results(ResultsAction),
    /// The single-cell edit overlay.
    CellEdit(CellEditAction),
    /// The SQL editor pane.
    Editor(EditorAction),
    /// The autocomplete popup.
    Completion(CompletionAction),
    /// The saved-connection list.
    Connections(ConnectionsAction),
    /// The add/edit connection form.
    Form(FormAction),
    /// The table browser.
    Tables(TablesAction),
    /// The results filter bar.
    Filter(FilterAction),
    /// The full-screen schema diagram.
    Diagram(DiagramAction),

    // -- Cross-cutting --
    /// Say something in the status bar. The reducer stamps the time, so key
    /// handlers stay pure and do not need a clock.
    Inform(String),
    /// Take whatever the status bar is saying down.
    DismissNotice,
    /// Show the full text of the current notice, cause and hint included.
    ShowNoticeDetail,
    /// Close that overlay, leaving the notice itself in the bar.
    CloseNoticeDetail,
    Quit,
    Noop,

    /// Hand work to the core. The only way this layer performs I/O.
    ///
    /// Boxed because `CoreCommand` is an order of magnitude larger than any
    /// other variant, and would otherwise set the size of every `Action`.
    SendCommand(Box<CoreCommand>),
    /// Apply several actions in order.
    Batch(Vec<Action>),
}

#[derive(Debug)]
pub enum NavAction {
    FocusPanel(FocusedPanel),
    SetNavMode(NavMode),
    SetEditorMode(EditorMode),
    ToggleSidebar,
    ClearPendingG,
    SetPendingG,
    ClearPendingD,
    SetPendingD,
    SetPendingLeader(bool),
}

#[derive(Debug)]
pub enum ResultsAction {
    RowDown,
    RowUp,
    ColRight,
    ColLeft,
    RowFirst,
    RowLast,
    HalfPageDown,
    HalfPageUp,
    ColFirst,
    ColLast,
    SetRow(usize),
    SetCol(usize),
    ToggleSort,
    MarkRowForDeletion,
    CommitPending,
    DiscardPendingOrEsc,
}

#[derive(Debug)]
pub enum CellEditAction {
    Enter,
    Stage,
    Cancel,
    Input(Input),
}

#[derive(Debug)]
pub enum EditorAction {
    Input(Input),
    CursorMove(CursorMove),
    RunQuery,
}

#[derive(Debug)]
pub enum CompletionAction {
    Up,
    Down,
    Accept,
    Dismiss,
}

#[derive(Debug)]
pub enum ConnectionsAction {
    Select(usize),
    ConnectSelected,
    DisconnectActive,
    OpenNewForm,
    OpenEditForm,
    InitDelete,
    ConfirmDelete,
    CancelDelete,
}

#[derive(Debug)]
pub enum FormAction {
    Close,
    NextField,
    PrevField,
    Input(char),
    Backspace,
    CycleBackend,
    CycleSsl,
    Submit,
}

#[derive(Debug)]
pub enum TablesAction {
    Select(usize),
    OpenSelected,
}

#[derive(Debug)]
pub enum FilterAction {
    Open,
    Close,
    CloseSuggestions,
    Input(Input),
    SuggestionUp,
    SuggestionDown,
    ApplySuggestion,
    Apply,
}

#[derive(Debug)]
pub enum DiagramAction {
    Open,
    Close,
    Scroll { dx: i16, dy: i16 },
    SelectNext,
    SelectPrev,
    SelectFirst,
    SelectLast,
    ToggleFocus,
    ToggleGlyph,
    JumpToTable,
    SearchOpen,
    SearchClose,
    SearchInput(char),
    SearchBackspace,
    SearchConfirm,
}

impl Action {
    /// Wrap a core command for dispatch.
    pub fn send(cmd: CoreCommand) -> Self {
        Action::SendCommand(Box::new(cmd))
    }
}

/// Apply an action to state and send any commands.
/// Apply an action to state and send any commands.
///
/// This is only a router: each domain owns its own reducer below, so adding a
/// case means touching one match instead of one 500-line one.
pub fn apply(action: Action, state: &mut AppState, cmd_tx: &mpsc::UnboundedSender<CoreCommand>) {
    match action {
        Action::Nav(a) => nav::apply(a, state, cmd_tx),
        Action::Results(a) => results::apply(a, state, cmd_tx),
        Action::CellEdit(a) => cell_edit::apply(a, state, cmd_tx),
        Action::Editor(a) => editor::apply(a, state, cmd_tx),
        Action::Completion(a) => completion_ui::apply(a, state, cmd_tx),
        Action::Connections(a) => connections::apply(a, state, cmd_tx),
        Action::Form(a) => form::apply(a, state, cmd_tx),
        Action::Tables(a) => tables::apply(a, state, cmd_tx),
        Action::Filter(a) => filter::apply(a, state, cmd_tx),
        Action::Diagram(a) => diagram::apply(a, state, cmd_tx),

        // -- Status --
        Action::Inform(msg) => {
            state.inform(msg);
        }

        Action::DismissNotice => {
            state.dismiss_notice();
        }

        Action::ShowNoticeDetail => {
            // Only worth an overlay if there is something the bar could not
            // fit. Otherwise this is a key press that appears to do nothing.
            if state.notice.as_ref().is_some_and(|n| n.has_detail()) {
                state.close_overlays();
                state.notice_detail_open = true;
            }
        }

        Action::CloseNoticeDetail => {
            state.notice_detail_open = false;
        }

        Action::Quit => {
            state.should_quit = true;
        }

        Action::Noop => {}

        // -- Side effects --
        Action::SendCommand(cmd) => {
            let cmd = *cmd;
            // Every arriving result set discards staged edits/deletes, so a
            // page change while changes are staged would silently destroy
            // them. This is the one choke point all paging keys go through.
            if matches!(cmd, CoreCommand::FetchPage { .. }) && results::has_staged_changes(state) {
                results::warn_staged_changes_block_paging(state);
                return;
            }
            let _ = cmd_tx.send(cmd);
        }

        Action::Batch(actions) => {
            for a in actions {
                apply(a, state, cmd_tx);
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn parse_filter_input_test(input: &str) -> Option<(String, &str)> {
    filter::parse_filter_input(input)
}

/// Apply live filter if the debounce deadline has passed. Called from tick.
/// Returns `true` if a filter was actually applied (state changed).
pub fn apply_live_filter_if_due(
    state: &mut AppState,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) -> bool {
    if !state.filter.visible {
        return false;
    }
    let Some(deadline) = state.filter.pending_live_apply_at else {
        return false;
    };
    if std::time::Instant::now() < deadline {
        return false;
    }
    state.filter.pending_live_apply_at = None;

    let query = state.filter.textarea.lines().join("");
    let trimmed = query.trim().to_owned();
    let is_candidate = filter::parse_filter_input(&trimmed)
        .map(|(_, value)| !value.trim().is_empty())
        .unwrap_or(false);

    if !is_candidate {
        if state.filter.last_applied_query.is_some() {
            state.filter.last_applied_query = None;
            state.active_filter = None;
            let _ = cmd_tx.send(CoreCommand::ClearFilter);
            return true;
        }
        return false;
    }

    if state.filter.last_applied_query.as_deref() == Some(trimmed.as_str()) {
        return false;
    }

    state.filter.last_applied_query = Some(trimmed.clone());
    state.active_filter = Some(trimmed.clone());
    let _ = cmd_tx.send(CoreCommand::ApplyFilter { query: trimmed });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{
        AppState, DiagramGlyphMode, DiagramState, EditorMode, FocusedPanel, NavMode, PendingDelete,
        PendingEdit,
    };
    use crate::test_helpers::{cmd_channel, make_state_with_results};
    use sbql_core::{CoreCommand, DiagramData};

    // -----------------------------------------------------------------------
    // Navigation
    // -----------------------------------------------------------------------

    #[test]
    fn focus_panel_normal() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Nav(NavAction::FocusPanel(FocusedPanel::Results)),
            &mut state,
            &tx,
        );
        assert_eq!(state.focused, FocusedPanel::Results);
    }

    #[test]
    fn focus_panel_sidebar_hidden_redirects() {
        let mut state = AppState::new(vec![]);
        state.layout.sidebar_hidden = true;
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Nav(NavAction::FocusPanel(FocusedPanel::Connections)),
            &mut state,
            &tx,
        );
        assert_eq!(state.focused, FocusedPanel::Editor);
    }

    #[test]
    fn focus_panel_sidebar_hidden_tables_redirects() {
        let mut state = AppState::new(vec![]);
        state.layout.sidebar_hidden = true;
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Nav(NavAction::FocusPanel(FocusedPanel::Tables)),
            &mut state,
            &tx,
        );
        assert_eq!(state.focused, FocusedPanel::Editor);
    }

    #[test]
    fn set_nav_mode() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Nav(NavAction::SetNavMode(NavMode::Panel)),
            &mut state,
            &tx,
        );
        assert_eq!(state.vim.nav_mode, NavMode::Panel);
    }

    #[test]
    fn set_editor_mode() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Nav(NavAction::SetEditorMode(EditorMode::Insert)),
            &mut state,
            &tx,
        );
        assert_eq!(state.editor.mode, EditorMode::Insert);
    }

    #[test]
    fn toggle_sidebar_hides() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::Nav(NavAction::ToggleSidebar), &mut state, &tx);
        assert!(state.layout.sidebar_hidden);
        assert_eq!(state.notice_text(), Some("Sidebar hidden"));
    }

    #[test]
    fn toggle_sidebar_shows() {
        let mut state = AppState::new(vec![]);
        state.layout.sidebar_hidden = true;
        let (tx, _rx) = cmd_channel();
        apply(Action::Nav(NavAction::ToggleSidebar), &mut state, &tx);
        assert!(!state.layout.sidebar_hidden);
        assert_eq!(state.notice_text(), Some("Sidebar shown"));
    }

    #[test]
    fn toggle_sidebar_hidden_redirects_focus() {
        let mut state = AppState::new(vec![]);
        state.focused = FocusedPanel::Connections;
        let (tx, _rx) = cmd_channel();
        apply(Action::Nav(NavAction::ToggleSidebar), &mut state, &tx);
        assert_eq!(state.focused, FocusedPanel::Editor);
    }

    // -----------------------------------------------------------------------
    // Results navigation
    // -----------------------------------------------------------------------

    #[test]
    fn move_row_down() {
        let mut state = make_state_with_results();
        let (tx, _rx) = cmd_channel();
        apply(Action::Results(ResultsAction::RowDown), &mut state, &tx);
        assert_eq!(state.results.selected_row, 1);
    }

    #[test]
    fn move_row_down_triggers_page_fetch() {
        let mut state = make_state_with_results();
        state.results.data.has_next_page = true;
        state.results.selected_row = 4; // last row
        let (tx, mut rx) = cmd_channel();
        apply(Action::Results(ResultsAction::RowDown), &mut state, &tx);
        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, CoreCommand::FetchPage { page: 1 }));
    }

    #[test]
    fn move_row_up() {
        let mut state = make_state_with_results();
        state.results.selected_row = 3;
        let (tx, _rx) = cmd_channel();
        apply(Action::Results(ResultsAction::RowUp), &mut state, &tx);
        assert_eq!(state.results.selected_row, 2);
    }

    #[test]
    fn move_col_right() {
        let mut state = make_state_with_results();
        let (tx, _rx) = cmd_channel();
        apply(Action::Results(ResultsAction::ColRight), &mut state, &tx);
        assert_eq!(state.results.selected_col, 1);
    }

    #[test]
    fn move_col_left() {
        let mut state = make_state_with_results();
        state.results.selected_col = 2;
        let (tx, _rx) = cmd_channel();
        apply(Action::Results(ResultsAction::ColLeft), &mut state, &tx);
        assert_eq!(state.results.selected_col, 1);
    }

    #[test]
    fn move_row_first() {
        let mut state = make_state_with_results();
        state.results.selected_row = 3;
        let (tx, _rx) = cmd_channel();
        apply(Action::Results(ResultsAction::RowFirst), &mut state, &tx);
        assert_eq!(state.results.selected_row, 0);
    }

    #[test]
    fn move_row_last() {
        let mut state = make_state_with_results();
        let (tx, _rx) = cmd_channel();
        apply(Action::Results(ResultsAction::RowLast), &mut state, &tx);
        assert_eq!(state.results.selected_row, 4);
    }

    #[test]
    fn move_half_page_down() {
        let mut state = make_state_with_results();
        state.results.viewport_height = 4;
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Results(ResultsAction::HalfPageDown),
            &mut state,
            &tx,
        );
        assert_eq!(state.results.selected_row, 2);
    }

    #[test]
    fn move_half_page_up() {
        let mut state = make_state_with_results();
        state.results.selected_row = 4;
        state.results.viewport_height = 4;
        let (tx, _rx) = cmd_channel();
        apply(Action::Results(ResultsAction::HalfPageUp), &mut state, &tx);
        assert_eq!(state.results.selected_row, 2);
    }

    #[test]
    fn move_col_first() {
        let mut state = make_state_with_results();
        state.results.selected_col = 2;
        let (tx, _rx) = cmd_channel();
        apply(Action::Results(ResultsAction::ColFirst), &mut state, &tx);
        assert_eq!(state.results.selected_col, 0);
    }

    #[test]
    fn move_col_last() {
        let mut state = make_state_with_results();
        let (tx, _rx) = cmd_channel();
        apply(Action::Results(ResultsAction::ColLast), &mut state, &tx);
        assert_eq!(state.results.selected_col, 2);
    }

    #[test]
    fn set_results_row_in_bounds() {
        let mut state = make_state_with_results();
        let (tx, _rx) = cmd_channel();
        apply(Action::Results(ResultsAction::SetRow(3)), &mut state, &tx);
        assert_eq!(state.results.selected_row, 3);
    }

    #[test]
    fn set_results_row_out_of_bounds_no_change() {
        let mut state = make_state_with_results();
        let (tx, _rx) = cmd_channel();
        apply(Action::Results(ResultsAction::SetRow(100)), &mut state, &tx);
        assert_eq!(state.results.selected_row, 0);
    }

    #[test]
    fn set_results_col_clamped() {
        let mut state = make_state_with_results();
        let (tx, _rx) = cmd_channel();
        apply(Action::Results(ResultsAction::SetCol(100)), &mut state, &tx);
        assert_eq!(state.results.selected_col, 2); // max col = 2
    }

    // -----------------------------------------------------------------------
    // Cell edit
    // -----------------------------------------------------------------------

    #[test]
    fn cancel_cell_edit() {
        let mut state = make_state_with_results();
        state.mutation.cell_edit = Some(crate::app::CellEditState::new(
            0,
            0,
            "id".into(),
            "1".into(),
            "public".into(),
            "users".into(),
            vec![("id".into(), "1".into())],
        ));
        let (tx, _rx) = cmd_channel();
        apply(Action::CellEdit(CellEditAction::Cancel), &mut state, &tx);
        assert!(state.mutation.cell_edit.is_none());
    }

    #[test]
    fn stage_cell_edit_with_change() {
        let mut state = make_state_with_results();
        let mut ce = crate::app::CellEditState::new(
            0,
            0,
            "id".into(),
            "1".into(),
            "public".into(),
            "users".into(),
            vec![("id".into(), "1".into())],
        );
        ce.textarea = tui_textarea::TextArea::default();
        ce.textarea.insert_str("999");
        state.mutation.cell_edit = Some(ce);
        let (tx, _rx) = cmd_channel();
        apply(Action::CellEdit(CellEditAction::Stage), &mut state, &tx);
        assert!(state.mutation.cell_edit.is_none());
        assert_eq!(state.mutation.pending_edits.len(), 1);
    }

    #[test]
    fn stage_cell_edit_unchanged() {
        let mut state = make_state_with_results();
        let ce = crate::app::CellEditState::new(
            0,
            0,
            "id".into(),
            "1".into(),
            "public".into(),
            "users".into(),
            vec![("id".into(), "1".into())],
        );
        state.mutation.cell_edit = Some(ce);
        let (tx, _rx) = cmd_channel();
        apply(Action::CellEdit(CellEditAction::Stage), &mut state, &tx);
        assert!(state.mutation.pending_edits.is_empty());
        assert_eq!(
            state.notice_text(),
            Some("No changes to stage (value unchanged).")
        );
    }

    // -----------------------------------------------------------------------
    // Commit pending
    // -----------------------------------------------------------------------

    #[test]
    fn commit_pending_empty_shows_error() {
        let mut state = make_state_with_results();
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Results(ResultsAction::CommitPending),
            &mut state,
            &tx,
        );
        assert!(state.is_failing());
    }

    #[test]
    fn commit_pending_sends_commands() {
        let mut state = make_state_with_results();
        state.mutation.pending_edits.insert(
            (0, 1),
            PendingEdit {
                new_val: "new".into(),
                schema: "public".into(),
                table: "users".into(),
                pk: vec![("id".into(), "1".into())],
                col_name: "name".into(),
            },
        );
        state.mutation.pending_deletes.insert(
            2,
            PendingDelete {
                schema: "public".into(),
                table: "users".into(),
                pk: vec![("id".into(), "3".into())],
            },
        );
        let (tx, mut rx) = cmd_channel();
        apply(
            Action::Results(ResultsAction::CommitPending),
            &mut state,
            &tx,
        );
        assert!(state.mutation.pending_edits.is_empty());
        assert!(state.mutation.pending_deletes.is_empty());
        // Should have sent UpdateCell, DeleteRow, FetchPage
        let mut cmds = Vec::new();
        while let Ok(cmd) = rx.try_recv() {
            cmds.push(cmd);
        }
        assert!(cmds.len() >= 3);
    }

    /// Fetching another page replaces the rows staged changes refer to and
    /// discards them — so paging is refused while anything is staged.
    #[test]
    fn paging_is_blocked_while_changes_are_staged() {
        let mut state = make_state_with_results();
        state.results.data.has_next_page = true;
        state.results.selected_row = state.results.data.rows.len() - 1;
        state.mutation.pending_edits.insert(
            (0, 0),
            PendingEdit {
                new_val: "x".into(),
                schema: "public".into(),
                table: "users".into(),
                pk: vec![("id".into(), "1".into())],
                col_name: "name".into(),
            },
        );
        let (tx, mut rx) = cmd_channel();

        // Cursor-past-bottom auto-fetch.
        apply(Action::Results(ResultsAction::RowDown), &mut state, &tx);
        assert!(rx.try_recv().is_err(), "RowDown must not fetch a page");

        // Explicit page keys go through SendCommand.
        apply(
            Action::send(CoreCommand::FetchPage { page: 1 }),
            &mut state,
            &tx,
        );
        assert!(rx.try_recv().is_err(), "FetchPage must be refused");
        assert!(state.notice_text().is_some(), "the refusal must be said");
    }

    /// The delete targets the table of the query that produced the rows, not
    /// whatever is currently typed (and unexecuted) in the editor.
    #[test]
    fn mark_for_deletion_targets_the_executed_query_table() {
        let mut state = make_state_with_results();
        state.results.source_sql = Some("SELECT * FROM orders".into());
        state.editor.textarea.insert_str("SELECT * FROM users");
        let (tx, mut rx) = cmd_channel();
        apply(
            Action::Results(ResultsAction::MarkRowForDeletion),
            &mut state,
            &tx,
        );
        match rx.try_recv().expect("a GetPrimaryKeys command") {
            CoreCommand::GetPrimaryKeys { table, .. } => assert_eq!(table, "orders"),
            other => panic!("expected GetPrimaryKeys, got {other:?}"),
        }
    }

    #[test]
    fn mark_for_deletion_without_an_executed_query_reports() {
        let mut state = make_state_with_results();
        state.results.source_sql = None;
        let (tx, mut rx) = cmd_channel();
        apply(
            Action::Results(ResultsAction::MarkRowForDeletion),
            &mut state,
            &tx,
        );
        assert!(rx.try_recv().is_err(), "no command without a source query");
        assert!(state.is_failing());
    }

    // -----------------------------------------------------------------------
    // Discard pending
    // -----------------------------------------------------------------------

    #[test]
    fn discard_pending_with_edits() {
        let mut state = make_state_with_results();
        state.mutation.pending_edits.insert(
            (0, 0),
            PendingEdit {
                new_val: "x".into(),
                schema: "p".into(),
                table: "t".into(),
                pk: vec![("id".into(), "1".into())],
                col_name: "c".into(),
            },
        );
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Results(ResultsAction::DiscardPendingOrEsc),
            &mut state,
            &tx,
        );
        assert!(state.mutation.pending_edits.is_empty());
        assert!(state.notice_text().unwrap().contains("discarded"));
    }

    #[test]
    fn discard_pending_empty_focuses_editor() {
        let mut state = make_state_with_results();
        state.focused = FocusedPanel::Results;
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Results(ResultsAction::DiscardPendingOrEsc),
            &mut state,
            &tx,
        );
        assert_eq!(state.focused, FocusedPanel::Editor);
    }

    // -----------------------------------------------------------------------
    // Editor
    // -----------------------------------------------------------------------

    #[test]
    fn run_query_sends_command() {
        let mut state = make_state_with_results();
        state.editor.textarea.insert_str("SELECT 1");
        let (tx, mut rx) = cmd_channel();
        apply(Action::Editor(EditorAction::RunQuery), &mut state, &tx);
        assert_eq!(state.focused, FocusedPanel::Results);
        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, CoreCommand::ExecuteQuery { .. }));
    }

    #[test]
    fn run_query_empty_noop() {
        let mut state = make_state_with_results();
        state.focused = FocusedPanel::Editor;
        let (tx, mut rx) = cmd_channel();
        apply(Action::Editor(EditorAction::RunQuery), &mut state, &tx);
        assert!(rx.try_recv().is_err());
        assert_eq!(state.focused, FocusedPanel::Editor);
    }

    // -----------------------------------------------------------------------
    // Connections
    // -----------------------------------------------------------------------

    #[test]
    fn select_connection() {
        let mut state = AppState::new(vec![
            sbql_core::ConnectionConfig::new_postgres("a", "h", 5432, "u", "d"),
            sbql_core::ConnectionConfig::new_postgres("b", "h", 5432, "u", "d"),
        ]);
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Connections(ConnectionsAction::Select(1)),
            &mut state,
            &tx,
        );
        assert_eq!(state.conn.selected(), 1);
    }

    #[test]
    fn select_connection_clamped() {
        let mut state = AppState::new(vec![sbql_core::ConnectionConfig::new_postgres(
            "a", "h", 5432, "u", "d",
        )]);
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Connections(ConnectionsAction::Select(10)),
            &mut state,
            &tx,
        );
        assert_eq!(state.conn.selected(), 0);
    }

    #[test]
    fn connect_selected_sends_command() {
        let cfg = sbql_core::ConnectionConfig::new_postgres("a", "h", 5432, "u", "d");
        let id = cfg.id;
        let mut state = AppState::new(vec![cfg]);
        let (tx, mut rx) = cmd_channel();
        apply(
            Action::Connections(ConnectionsAction::ConnectSelected),
            &mut state,
            &tx,
        );
        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, CoreCommand::Connect(cid) if cid == id));
    }

    #[test]
    fn open_new_form() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Connections(ConnectionsAction::OpenNewForm),
            &mut state,
            &tx,
        );
        assert!(state.conn.form.visible);
    }

    #[test]
    fn open_edit_form() {
        let mut state = AppState::new(vec![sbql_core::ConnectionConfig::new_postgres(
            "a",
            "localhost",
            5432,
            "u",
            "d",
        )]);
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Connections(ConnectionsAction::OpenEditForm),
            &mut state,
            &tx,
        );
        assert!(state.conn.form.visible);
        assert_eq!(state.conn.form.draft.name, "a");
    }

    #[test]
    fn init_delete_connection() {
        let mut state = AppState::new(vec![sbql_core::ConnectionConfig::new_postgres(
            "myconn", "h", 5432, "u", "d",
        )]);
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Connections(ConnectionsAction::InitDelete),
            &mut state,
            &tx,
        );
        assert!(state.conn.pending_delete.is_some());
    }

    #[test]
    fn confirm_delete_connection() {
        let mut state = AppState::new(vec![]);
        state.conn.pending_delete = Some((uuid::Uuid::new_v4(), "test".into()));
        let (tx, mut rx) = cmd_channel();
        apply(
            Action::Connections(ConnectionsAction::ConfirmDelete),
            &mut state,
            &tx,
        );
        assert!(state.conn.pending_delete.is_none());
        assert!(rx.try_recv().is_ok());
    }

    #[test]
    fn cancel_delete_connection() {
        let mut state = AppState::new(vec![]);
        state.conn.pending_delete = Some((uuid::Uuid::new_v4(), "test".into()));
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Connections(ConnectionsAction::CancelDelete),
            &mut state,
            &tx,
        );
        assert!(state.conn.pending_delete.is_none());
    }

    #[test]
    fn disconnect_active() {
        let mut state = AppState::new(vec![]);
        let id = uuid::Uuid::new_v4();
        state.conn.active_id = Some(id);
        let (tx, mut rx) = cmd_channel();
        apply(
            Action::Connections(ConnectionsAction::DisconnectActive),
            &mut state,
            &tx,
        );
        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, CoreCommand::Disconnect(d) if d == id));
    }

    // -----------------------------------------------------------------------
    // Connection form
    // -----------------------------------------------------------------------

    #[test]
    fn form_close() {
        let mut state = AppState::new(vec![]);
        state.conn.form.visible = true;
        let (tx, _rx) = cmd_channel();
        apply(Action::Form(FormAction::Close), &mut state, &tx);
        assert!(!state.conn.form.visible);
    }

    #[test]
    fn form_next_field_wraps() {
        let mut state = AppState::new(vec![]);
        state.conn.form.field_index = 7; // last PG field (SSL Mode)
        let (tx, _rx) = cmd_channel();
        apply(Action::Form(FormAction::NextField), &mut state, &tx);
        assert_eq!(state.conn.form.field_index, 0);
    }

    #[test]
    fn form_prev_field_wraps() {
        let mut state = AppState::new(vec![]);
        state.conn.form.field_index = 0;
        let (tx, _rx) = cmd_channel();
        apply(Action::Form(FormAction::PrevField), &mut state, &tx);
        assert_eq!(state.conn.form.field_index, 7); // wraps to last PG field
    }

    #[test]
    fn form_input_char() {
        let mut state = AppState::new(vec![]);
        state.conn.form.field_index = 1; // Name field
        let (tx, _rx) = cmd_channel();
        apply(Action::Form(FormAction::Input('a')), &mut state, &tx);
        assert_eq!(state.conn.form.draft.name, "a");
    }

    #[test]
    fn form_backspace() {
        let mut state = AppState::new(vec![]);
        state.conn.form.draft.name = "ab".into();
        state.conn.form.field_index = 1; // Name field
        let (tx, _rx) = cmd_channel();
        apply(Action::Form(FormAction::Backspace), &mut state, &tx);
        assert_eq!(state.conn.form.draft.name, "a");
    }

    #[test]
    fn form_cycle_ssl() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::Form(FormAction::CycleSsl), &mut state, &tx);
        assert_eq!(state.conn.form.draft.ssl_mode, sbql_core::SslMode::Require);
    }

    #[test]
    fn form_submit_valid() {
        let mut state = AppState::new(vec![]);
        state.conn.form.visible = true;
        state.conn.form.draft.name = "test".into();
        state.conn.form.draft.host = "localhost".into();
        state.conn.form.draft.port = "5432".into();
        state.conn.form.draft.user = "postgres".into();
        state.conn.form.draft.database = "testdb".into();
        let (tx, mut rx) = cmd_channel();
        apply(Action::Form(FormAction::Submit), &mut state, &tx);
        assert!(!state.conn.form.visible);
        assert!(rx.try_recv().is_ok());
    }

    #[test]
    fn form_submit_missing_name() {
        let mut state = AppState::new(vec![]);
        state.conn.form.visible = true;
        state.conn.form.draft.name = "".into();
        let (tx, _rx) = cmd_channel();
        apply(Action::Form(FormAction::Submit), &mut state, &tx);
        assert!(state.conn.form.error.is_some());
        assert!(state.conn.form.visible);
    }

    #[test]
    fn form_submit_bad_port() {
        let mut state = AppState::new(vec![]);
        state.conn.form.visible = true;
        state.conn.form.draft.name = "test".into();
        state.conn.form.draft.host = "localhost".into();
        state.conn.form.draft.port = "not_a_number".into();
        state.conn.form.draft.user = "u".into();
        state.conn.form.draft.database = "d".into();
        let (tx, _rx) = cmd_channel();
        apply(Action::Form(FormAction::Submit), &mut state, &tx);
        assert!(state.conn.form.error.unwrap().contains("Port"));
    }

    #[test]
    fn form_submit_redis_valid() {
        let mut state = AppState::new(vec![]);
        state.conn.form.visible = true;
        state.conn.form.draft.backend = sbql_core::DbBackend::Redis;
        state.conn.form.draft.name = "my-redis".into();
        state.conn.form.draft.host = "localhost".into();
        state.conn.form.draft.port = "6379".into();
        state.conn.form.draft.database = "0".into();
        let (tx, mut rx) = cmd_channel();
        apply(Action::Form(FormAction::Submit), &mut state, &tx);
        assert!(!state.conn.form.visible);
        assert!(state.conn.form.error.is_none());
        assert!(rx.try_recv().is_ok());
    }

    #[test]
    fn form_submit_redis_missing_host() {
        let mut state = AppState::new(vec![]);
        state.conn.form.visible = true;
        state.conn.form.draft.backend = sbql_core::DbBackend::Redis;
        state.conn.form.draft.name = "my-redis".into();
        state.conn.form.draft.host = "".into();
        state.conn.form.draft.port = "6379".into();
        let (tx, _rx) = cmd_channel();
        apply(Action::Form(FormAction::Submit), &mut state, &tx);
        assert!(state.conn.form.visible);
        assert!(state.conn.form.error.as_ref().unwrap().contains("Host"));
    }

    #[test]
    fn form_submit_redis_bad_port() {
        let mut state = AppState::new(vec![]);
        state.conn.form.visible = true;
        state.conn.form.draft.backend = sbql_core::DbBackend::Redis;
        state.conn.form.draft.name = "my-redis".into();
        state.conn.form.draft.host = "localhost".into();
        state.conn.form.draft.port = "abc".into();
        let (tx, _rx) = cmd_channel();
        apply(Action::Form(FormAction::Submit), &mut state, &tx);
        assert!(state.conn.form.visible);
        assert!(state.conn.form.error.as_ref().unwrap().contains("Port"));
    }

    // -----------------------------------------------------------------------
    // Tables
    // -----------------------------------------------------------------------

    #[test]
    fn select_table() {
        let mut state = AppState::new(vec![]);
        state.tables.tables = (0..6)
            .map(|i| sbql_core::TableEntry {
                schema: "public".into(),
                name: format!("t{i}"),
            })
            .collect();
        let (tx, _rx) = cmd_channel();
        apply(Action::Tables(TablesAction::Select(5)), &mut state, &tx);
        assert_eq!(state.tables.selected(), 5);
    }

    /// A click below the last row used to store an index past the end, leaving
    /// the panel with nothing selected.
    #[test]
    fn select_table_past_the_end_lands_on_the_last_row() {
        let mut state = AppState::new(vec![]);
        state.tables.tables = vec![sbql_core::TableEntry {
            schema: "public".into(),
            name: "only".into(),
        }];
        let (tx, _rx) = cmd_channel();
        apply(Action::Tables(TablesAction::Select(99)), &mut state, &tx);
        assert_eq!(state.tables.selected(), 0);
        assert!(state.tables.tables.get(state.tables.selected()).is_some());
    }

    #[test]
    fn select_table_in_an_empty_list_stays_at_zero() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::Tables(TablesAction::Select(5)), &mut state, &tx);
        assert_eq!(state.tables.selected(), 0);
    }

    // -----------------------------------------------------------------------
    // Filter
    // -----------------------------------------------------------------------

    #[test]
    fn open_filter() {
        let mut state = make_state_with_results();
        let (tx, _rx) = cmd_channel();
        apply(Action::Filter(FilterAction::Open), &mut state, &tx);
        assert!(state.filter.visible);
    }

    #[test]
    fn filter_close() {
        let mut state = make_state_with_results();
        state.filter.visible = true;
        let (tx, mut rx) = cmd_channel();
        apply(Action::Filter(FilterAction::Close), &mut state, &tx);
        assert!(!state.filter.visible);
        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, CoreCommand::ClearFilter));
    }

    #[test]
    fn filter_close_suggestions() {
        let mut state = make_state_with_results();
        state.filter.show_suggestions = true;
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Filter(FilterAction::CloseSuggestions),
            &mut state,
            &tx,
        );
        assert!(!state.filter.show_suggestions);
    }

    #[test]
    fn filter_suggestion_up() {
        let mut state = make_state_with_results();
        state.filter.suggestions = vec!["a".into(), "b".into(), "c".into()];
        state.filter.suggestion_cursor.select(2, 3);
        let (tx, _rx) = cmd_channel();
        apply(Action::Filter(FilterAction::SuggestionUp), &mut state, &tx);
        assert_eq!(state.filter.suggestion_cursor.index(), 1);
    }

    #[test]
    fn filter_suggestion_down() {
        let mut state = make_state_with_results();
        state.filter.suggestions = vec!["a".into(), "b".into(), "c".into()];
        state.filter.suggestion_cursor.reset();
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Filter(FilterAction::SuggestionDown),
            &mut state,
            &tx,
        );
        assert_eq!(state.filter.suggestion_cursor.index(), 1);
    }

    #[test]
    fn filter_apply_with_query() {
        let mut state = make_state_with_results();
        state.filter.visible = true;
        state.filter.textarea = tui_textarea::TextArea::default();
        state.filter.textarea.insert_str("name:Alice");
        let (tx, mut rx) = cmd_channel();
        apply(Action::Filter(FilterAction::Apply), &mut state, &tx);
        assert!(!state.filter.visible);
        assert_eq!(state.active_filter, Some("name:Alice".into()));
        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, CoreCommand::ApplyFilter { .. }));
    }

    #[test]
    fn filter_apply_empty_clears() {
        let mut state = make_state_with_results();
        state.filter.visible = true;
        state.active_filter = Some("old".into());
        let (tx, mut rx) = cmd_channel();
        apply(Action::Filter(FilterAction::Apply), &mut state, &tx);
        assert!(state.active_filter.is_none());
        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, CoreCommand::ClearFilter));
    }

    // -----------------------------------------------------------------------
    // Diagram
    // -----------------------------------------------------------------------

    #[test]
    fn open_diagram_without_connection_shows_error() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::Diagram(DiagramAction::Open), &mut state, &tx);
        assert!(state.is_failing());
    }

    #[test]
    fn open_diagram_with_connection_sends_command() {
        let mut state = AppState::new(vec![]);
        state.conn.active_id = Some(uuid::Uuid::new_v4());
        let (tx, mut rx) = cmd_channel();
        apply(Action::Diagram(DiagramAction::Open), &mut state, &tx);
        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, CoreCommand::LoadDiagram));
    }

    #[test]
    fn close_diagram() {
        let mut state = AppState::new(vec![]);
        state.diagram = Some(DiagramState::new(DiagramData::default()));
        let (tx, _rx) = cmd_channel();
        apply(Action::Diagram(DiagramAction::Close), &mut state, &tx);
        assert!(state.diagram.is_none());
    }

    #[test]
    fn diagram_scroll() {
        let mut state = AppState::new(vec![]);
        state.diagram = Some(DiagramState::new(DiagramData::default()));
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Diagram(DiagramAction::Scroll { dx: 5, dy: 3 }),
            &mut state,
            &tx,
        );
        assert_eq!(state.diagram.as_ref().unwrap().scroll_x, 5);
        assert_eq!(state.diagram.as_ref().unwrap().scroll_y, 3);
    }

    #[test]
    fn diagram_toggle_glyph() {
        let mut state = AppState::new(vec![]);
        state.diagram = Some(DiagramState::new(DiagramData::default()));
        let (tx, _rx) = cmd_channel();
        apply(Action::Diagram(DiagramAction::ToggleGlyph), &mut state, &tx);
        assert_eq!(
            state.diagram.as_ref().unwrap().glyph_mode,
            DiagramGlyphMode::Unicode
        );
    }

    #[test]
    fn diagram_toggle_focus() {
        let mut state = AppState::new(vec![]);
        state.diagram = Some(DiagramState::new(DiagramData::default()));
        let (tx, _rx) = cmd_channel();
        apply(Action::Diagram(DiagramAction::ToggleFocus), &mut state, &tx);
        assert!(state.diagram.as_ref().unwrap().focus_mode);
    }

    // -----------------------------------------------------------------------
    // Vim state
    // -----------------------------------------------------------------------

    #[test]
    fn set_pending_g() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::Nav(NavAction::SetPendingG), &mut state, &tx);
        assert!(state.vim.pending_g);
    }

    #[test]
    fn clear_pending_g() {
        let mut state = AppState::new(vec![]);
        state.vim.pending_g = true;
        let (tx, _rx) = cmd_channel();
        apply(Action::Nav(NavAction::ClearPendingG), &mut state, &tx);
        assert!(!state.vim.pending_g);
    }

    #[test]
    fn set_pending_d() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::Nav(NavAction::SetPendingD), &mut state, &tx);
        assert!(state.mutation.pending_d);
    }

    #[test]
    fn clear_pending_d() {
        let mut state = AppState::new(vec![]);
        state.mutation.pending_d = true;
        let (tx, _rx) = cmd_channel();
        apply(Action::Nav(NavAction::ClearPendingD), &mut state, &tx);
        assert!(!state.mutation.pending_d);
    }

    #[test]
    fn set_pending_leader() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Nav(NavAction::SetPendingLeader(true)),
            &mut state,
            &tx,
        );
        assert!(state.vim.pending_leader);
    }

    // -----------------------------------------------------------------------
    // Status
    // -----------------------------------------------------------------------

    #[test]
    fn informing_the_user_replaces_whatever_the_bar_was_saying() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();

        state.report("something went wrong");
        assert!(state.is_failing());

        apply(Action::Inform("hello".into()), &mut state, &tx);
        assert_eq!(state.notice_text(), Some("hello"));
        // The bug one field was introduced to make impossible: a stale failure
        // outranking the thing that happened after it.
        assert!(!state.is_failing());
    }

    #[test]
    fn esc_takes_a_failure_down() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();

        state.report("err");
        apply(Action::DismissNotice, &mut state, &tx);
        assert_eq!(state.notice_text(), None);
    }

    /// Offering an overlay with nothing extra in it is a key press that looks
    /// broken, so the action declines.
    #[test]
    fn the_detail_overlay_only_opens_when_there_is_detail() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();

        state.inform("nothing more to say");
        apply(Action::ShowNoticeDetail, &mut state, &tx);
        assert!(!state.notice_detail_open);

        state.report_core(
            sbql_core::CoreError::new(sbql_core::ErrorKind::Query, "bad sql")
                .with_detail("near \")\""),
        );
        apply(Action::ShowNoticeDetail, &mut state, &tx);
        assert!(state.notice_detail_open);
        assert_eq!(state.mode(), crate::app::Mode::NoticeDetail);

        apply(Action::CloseNoticeDetail, &mut state, &tx);
        assert!(!state.notice_detail_open);
        assert!(state.is_failing(), "closing the overlay keeps the message");
    }

    #[test]
    fn quit() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::Quit, &mut state, &tx);
        assert!(state.should_quit);
    }

    #[test]
    fn noop_does_nothing() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::Noop, &mut state, &tx);
        assert!(!state.should_quit);
    }

    #[test]
    fn batch_applies_all() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Batch(vec![
                Action::Nav(NavAction::SetPendingG),
                Action::Nav(NavAction::SetPendingD),
            ]),
            &mut state,
            &tx,
        );
        assert!(state.vim.pending_g);
        assert!(state.mutation.pending_d);
    }

    #[test]
    fn send_command() {
        let mut state = AppState::new(vec![]);
        let (tx, mut rx) = cmd_channel();
        apply(Action::send(CoreCommand::ListTables), &mut state, &tx);
        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, CoreCommand::ListTables));
    }

    // -----------------------------------------------------------------------
    // parse_filter_input
    // -----------------------------------------------------------------------

    #[test]
    fn parse_filter_col_value() {
        let result = parse_filter_input_test("name:Alice");
        assert_eq!(result, Some(("name".into(), "Alice")));
    }

    #[test]
    fn parse_filter_empty_col() {
        let result = parse_filter_input_test(":value");
        assert!(result.is_none());
    }

    #[test]
    fn parse_filter_no_colon() {
        let result = parse_filter_input_test("plain text");
        assert!(result.is_none());
    }

    // -----------------------------------------------------------------------
    // apply_live_filter_if_due
    // -----------------------------------------------------------------------

    #[test]
    fn live_filter_not_visible_returns_false() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        assert!(!apply_live_filter_if_due(&mut state, &tx));
    }

    #[test]
    fn live_filter_no_deadline_returns_false() {
        let mut state = AppState::new(vec![]);
        state.filter.visible = true;
        let (tx, _rx) = cmd_channel();
        assert!(!apply_live_filter_if_due(&mut state, &tx));
    }

    #[test]
    fn live_filter_deadline_in_future_returns_false() {
        let mut state = AppState::new(vec![]);
        state.filter.visible = true;
        state.filter.pending_live_apply_at =
            Some(std::time::Instant::now() + std::time::Duration::from_secs(60));
        let (tx, _rx) = cmd_channel();
        assert!(!apply_live_filter_if_due(&mut state, &tx));
    }
}
