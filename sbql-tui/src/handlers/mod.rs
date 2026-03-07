pub mod cell_edit;
pub mod connections;
pub mod diagram;
pub mod editor;
pub mod filter;
pub mod mouse;
pub mod navigation;
pub mod results;
pub mod tables;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;
use crate::app::{AppState, EditorMode, FocusedPanel, NavMode};
use crate::events::is_quit;

/// Top-level key dispatch. Returns an [`Action`] to be applied by the event loop.
pub fn handle_key(state: &AppState, key: KeyEvent) -> Action {
    tracing::info!(
        "handle_key: focused={:?} code={:?} mods={:?} cell_edit={} conn_form={} filter={}",
        state.focused,
        key.code,
        key.modifiers,
        state.mutation.cell_edit.is_some(),
        state.conn.form.visible,
        state.filter.visible,
    );

    // ---- Diagram mode — intercept all keys ----
    if state.diagram.is_some() {
        return diagram::handle(state, key);
    }

    // ---- Cell edit mode ----
    if state.mutation.cell_edit.is_some() {
        return cell_edit::handle(state, key);
    }

    // ---- Filter bar mode ----
    if state.filter.visible {
        return filter::handle(state, key);
    }

    // ---- Connection form mode ----
    if state.conn.form.visible {
        return connections::handle_form(state, key);
    }

    // ---- Pending destructive confirmation ----
    if state.conn.pending_delete.is_some() {
        return connections::handle_confirm_delete(state, key);
    }

    // ---- Global keys ----
    if is_quit(&key) {
        return Action::Quit;
    }

    // Ctrl+\ — toggle sidebar visibility
    if key.code == KeyCode::Char('\\') && key.modifiers == KeyModifiers::CONTROL {
        return Action::ToggleSidebar;
    }

    // In Editor Insert mode, keep typing local to editor.
    if state.editor.mode == EditorMode::Insert && state.focused == FocusedPanel::Editor {
        if key.code == KeyCode::Esc {
            return Action::Batch(vec![
                Action::SetEditorMode(EditorMode::Normal),
                Action::SetNavMode(NavMode::Panel),
                Action::SetPendingLeader(false),
            ]);
        }
        return editor::handle(state, key);
    }

    // Esc leaves panel mode and returns to global mode.
    if key.code == KeyCode::Esc {
        let mut actions = vec![
            Action::SetPendingLeader(false),
            Action::SetEditorMode(EditorMode::Normal),
        ];
        if state.vim.nav_mode == NavMode::Panel {
            actions.push(Action::SetNavMode(NavMode::Global));
            actions.push(Action::SetStatus(Some("Global mode".into())));
            actions.push(Action::SetError(None));
        }
        return Action::Batch(actions);
    }

    // Reliable panel shortcuts: F1-F4 / Ctrl+1-4
    let focus_target = match (key.code, key.modifiers) {
        (KeyCode::F(1), _) | (KeyCode::Char('1'), KeyModifiers::CONTROL) => {
            Some(FocusedPanel::Connections)
        }
        (KeyCode::F(2), _) | (KeyCode::Char('2'), KeyModifiers::CONTROL) => {
            Some(FocusedPanel::Tables)
        }
        (KeyCode::F(3), _) | (KeyCode::Char('3'), KeyModifiers::CONTROL) => {
            Some(FocusedPanel::Editor)
        }
        (KeyCode::F(4), _) | (KeyCode::Char('4'), KeyModifiers::CONTROL) => {
            Some(FocusedPanel::Results)
        }
        _ => None,
    };
    if let Some(target) = focus_target {
        return Action::FocusPanel(target);
    }

    // Shift+D = open database diagram
    if key.code == KeyCode::Char('D') {
        return Action::OpenDiagram;
    }

    // Tab / BackTab cycles focus
    if key.code == KeyCode::Tab && key.modifiers == KeyModifiers::NONE {
        return Action::FocusPanel(navigation::tab_next(
            state.focused,
            state.layout.sidebar_hidden,
        ));
    }
    if key.code == KeyCode::BackTab {
        return Action::FocusPanel(navigation::tab_prev(
            state.focused,
            state.layout.sidebar_hidden,
        ));
    }

    if state.vim.nav_mode == NavMode::Global {
        if state.vim.pending_leader {
            return match (key.code, key.modifiers) {
                (KeyCode::Char('e'), KeyModifiers::NONE) => {
                    Action::Batch(vec![Action::SetPendingLeader(false), Action::ToggleSidebar])
                }
                _ => Action::Batch(vec![
                    Action::SetPendingLeader(false),
                    Action::SetStatus(Some("Unknown leader combo. Try: Space e".into())),
                ]),
            };
        }

        if key.code == KeyCode::Char(' ') && key.modifiers == KeyModifiers::NONE {
            return Action::Batch(vec![
                Action::SetPendingLeader(true),
                Action::SetStatus(Some("Leader: _  (e: toggle sidebar)".into())),
                Action::SetError(None),
            ]);
        }

        if key.code == KeyCode::Char('i') && key.modifiers == KeyModifiers::NONE {
            return match state.focused {
                FocusedPanel::Results => Action::Batch(vec![
                    Action::SetNavMode(NavMode::Panel),
                    Action::EnterCellEdit,
                ]),
                FocusedPanel::Editor => Action::Batch(vec![
                    Action::SetNavMode(NavMode::Panel),
                    Action::SetEditorMode(EditorMode::Insert),
                ]),
                FocusedPanel::Connections | FocusedPanel::Tables => Action::Batch(vec![
                    Action::FocusPanel(FocusedPanel::Editor),
                    Action::SetNavMode(NavMode::Panel),
                    Action::SetEditorMode(EditorMode::Insert),
                ]),
            };
        }

        if key.code == KeyCode::Enter {
            return Action::Batch(vec![
                Action::SetNavMode(NavMode::Panel),
                Action::SetStatus(Some("Panel mode".into())),
                Action::SetError(None),
            ]);
        }

        if let Some(action) = navigation::try_navigate_panels(state, key) {
            return action;
        }

        return Action::Noop;
    }

    // In panel mode, Alt+hjkl still navigates panel focus.
    if key.modifiers == KeyModifiers::ALT {
        if let Some(action) = navigation::try_navigate_panels(state, key) {
            return action;
        }
    }

    // In panel mode, `i` inside editor enters Insert mode.
    if state.focused == FocusedPanel::Editor
        && key.code == KeyCode::Char('i')
        && key.modifiers == KeyModifiers::NONE
        && state.editor.mode == EditorMode::Normal
    {
        return Action::SetEditorMode(EditorMode::Insert);
    }

    match state.focused {
        FocusedPanel::Connections => connections::handle(state, key),
        FocusedPanel::Tables => tables::handle(state, key),
        FocusedPanel::Editor => editor::handle(state, key),
        FocusedPanel::Results => results::handle(state, key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{CellEditState, DiagramState};
    use crate::test_helpers::{key, key_mod, make_state_with_results};
    use sbql_core::{DiagramData, TableEntry};

    // -- Priority: diagram intercepts all --

    #[test]
    fn diagram_mode_intercepts() {
        let mut state = make_state_with_results();
        state.diagram = Some(DiagramState::new(DiagramData::default()));
        let act = handle_key(&state, key(KeyCode::Char('q')));
        assert!(matches!(act, Action::CloseDiagram));
    }

    // -- Priority: cell edit --

    #[test]
    fn cell_edit_mode_intercepts() {
        let mut state = make_state_with_results();
        state.mutation.cell_edit = Some(CellEditState::new(
            0, 0, "id".into(), "1".into(), "public".into(), "users".into(), "id".into(), "1".into(),
        ));
        let act = handle_key(&state, key(KeyCode::Esc));
        assert!(matches!(act, Action::CancelCellEdit));
    }

    // -- Priority: filter --

    #[test]
    fn filter_mode_intercepts() {
        let mut state = make_state_with_results();
        state.filter.visible = true;
        let act = handle_key(&state, key(KeyCode::Esc));
        assert!(matches!(act, Action::FilterClose));
    }

    // -- Priority: connection form --

    #[test]
    fn form_mode_intercepts() {
        let mut state = make_state_with_results();
        state.conn.form.visible = true;
        let act = handle_key(&state, key(KeyCode::Esc));
        assert!(matches!(act, Action::FormClose));
    }

    // -- Priority: delete confirm --

    #[test]
    fn delete_confirm_intercepts() {
        let mut state = make_state_with_results();
        state.conn.pending_delete = Some((uuid::Uuid::new_v4(), "test".into()));
        let act = handle_key(&state, key(KeyCode::Char('y')));
        assert!(matches!(act, Action::ConfirmDeleteConnection));
    }

    // -- Quit --

    #[test]
    fn q_quits() {
        let state = make_state_with_results();
        let act = handle_key(&state, key(KeyCode::Char('q')));
        assert!(matches!(act, Action::Quit));
    }

    #[test]
    fn ctrl_c_quits() {
        let state = make_state_with_results();
        let act = handle_key(&state, key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(matches!(act, Action::Quit));
    }

    // -- Toggle sidebar --

    #[test]
    fn ctrl_backslash_toggles_sidebar() {
        let state = make_state_with_results();
        let act = handle_key(&state, key_mod(KeyCode::Char('\\'), KeyModifiers::CONTROL));
        assert!(matches!(act, Action::ToggleSidebar));
    }

    // -- Editor insert mode stays local --

    #[test]
    fn editor_insert_esc_returns_to_normal() {
        let mut state = make_state_with_results();
        state.focused = FocusedPanel::Editor;
        state.editor.mode = EditorMode::Insert;
        let act = handle_key(&state, key(KeyCode::Esc));
        assert!(matches!(act, Action::Batch(_)));
    }

    // -- F1-F4 focus shortcuts --

    #[test]
    fn f1_focuses_connections() {
        let mut state = make_state_with_results();
        state.focused = FocusedPanel::Editor;
        let act = handle_key(&state, key(KeyCode::F(1)));
        assert!(matches!(act, Action::FocusPanel(FocusedPanel::Connections)));
    }

    #[test]
    fn f2_focuses_tables() {
        let state = make_state_with_results();
        let act = handle_key(&state, key(KeyCode::F(2)));
        assert!(matches!(act, Action::FocusPanel(FocusedPanel::Tables)));
    }

    #[test]
    fn f3_focuses_editor() {
        let state = make_state_with_results();
        let act = handle_key(&state, key(KeyCode::F(3)));
        assert!(matches!(act, Action::FocusPanel(FocusedPanel::Editor)));
    }

    #[test]
    fn f4_focuses_results() {
        let state = make_state_with_results();
        let act = handle_key(&state, key(KeyCode::F(4)));
        assert!(matches!(act, Action::FocusPanel(FocusedPanel::Results)));
    }

    // -- Tab cycles --

    #[test]
    fn tab_cycles_forward() {
        let mut state = make_state_with_results();
        state.focused = FocusedPanel::Connections;
        let act = handle_key(&state, key(KeyCode::Tab));
        assert!(matches!(act, Action::FocusPanel(FocusedPanel::Tables)));
    }

    #[test]
    fn backtab_cycles_backward() {
        let mut state = make_state_with_results();
        state.focused = FocusedPanel::Tables;
        let act = handle_key(&state, key(KeyCode::BackTab));
        assert!(matches!(act, Action::FocusPanel(FocusedPanel::Connections)));
    }

    // -- Shift+D opens diagram --

    #[test]
    fn shift_d_opens_diagram() {
        let state = make_state_with_results();
        let act = handle_key(&state, key(KeyCode::Char('D')));
        assert!(matches!(act, Action::OpenDiagram));
    }

    // -- Global mode: Space leader --

    #[test]
    fn space_sets_pending_leader() {
        let mut state = make_state_with_results();
        state.vim.nav_mode = NavMode::Global;
        let act = handle_key(&state, key(KeyCode::Char(' ')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn leader_e_toggles_sidebar() {
        let mut state = make_state_with_results();
        state.vim.nav_mode = NavMode::Global;
        state.vim.pending_leader = true;
        let act = handle_key(&state, key(KeyCode::Char('e')));
        assert!(matches!(act, Action::Batch(_)));
    }

    // -- Enter in global enters panel mode --

    #[test]
    fn enter_in_global_enters_panel() {
        let mut state = make_state_with_results();
        state.vim.nav_mode = NavMode::Global;
        let act = handle_key(&state, key(KeyCode::Enter));
        assert!(matches!(act, Action::Batch(_)));
    }

    // -- i in global mode --

    #[test]
    fn i_global_on_editor_enters_insert() {
        let mut state = make_state_with_results();
        state.vim.nav_mode = NavMode::Global;
        state.focused = FocusedPanel::Editor;
        let act = handle_key(&state, key(KeyCode::Char('i')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn i_global_on_results_enters_cell_edit() {
        let mut state = make_state_with_results();
        state.vim.nav_mode = NavMode::Global;
        state.focused = FocusedPanel::Results;
        let act = handle_key(&state, key(KeyCode::Char('i')));
        assert!(matches!(act, Action::Batch(_)));
    }

    // -- Panel mode: Alt+hjkl navigation --

    #[test]
    fn alt_l_navigates_in_panel_mode() {
        let mut state = make_state_with_results();
        state.vim.nav_mode = NavMode::Panel;
        state.focused = FocusedPanel::Connections;
        let act = handle_key(&state, key_mod(KeyCode::Char('l'), KeyModifiers::ALT));
        assert!(matches!(act, Action::FocusPanel(FocusedPanel::Editor)));
    }

    // -- Panel mode: i enters insert in editor --

    #[test]
    fn i_panel_mode_editor_normal_enters_insert() {
        let mut state = make_state_with_results();
        state.vim.nav_mode = NavMode::Panel;
        state.focused = FocusedPanel::Editor;
        state.editor.mode = EditorMode::Normal;
        let act = handle_key(&state, key(KeyCode::Char('i')));
        assert!(matches!(act, Action::SetEditorMode(EditorMode::Insert)));
    }

    // -- Esc in panel mode returns to global --

    #[test]
    fn esc_in_panel_mode_returns_global() {
        let mut state = make_state_with_results();
        state.vim.nav_mode = NavMode::Panel;
        let act = handle_key(&state, key(KeyCode::Esc));
        assert!(matches!(act, Action::Batch(_)));
    }
}
