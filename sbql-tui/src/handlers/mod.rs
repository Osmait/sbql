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

use crate::action::{Action, CellEditAction, DiagramAction, NavAction};
use crate::app::{AppState, EditorMode, FocusedPanel, Mode, NavMode};
use crate::events::is_quit;

/// Top-level key dispatch. Returns an [`Action`] to be applied by the event loop.
pub fn handle_key(state: &AppState, key: KeyEvent) -> Action {
    tracing::info!(
        "handle_key: mode={:?} focused={:?} code={:?} mods={:?}",
        state.mode(),
        state.focused,
        key.code,
        key.modifiers,
    );

    // Overlays own the keyboard while they are open. `mode()` decides which
    // one wins; this match is exhaustive, so a new overlay cannot be added
    // without deciding what it does with a key press.
    match state.mode() {
        Mode::Diagram => return diagram::handle(state, key),
        // A reader, not an editor: any key closes it, so it is not another
        // mode the user has to find their way out of.
        Mode::NoticeDetail => return Action::CloseNoticeDetail,
        Mode::CellEdit => return cell_edit::handle(state, key),
        Mode::Filter => return filter::handle(state, key),
        Mode::ConnectionForm => return connections::handle_form(state, key),
        Mode::ConfirmDelete => return connections::handle_confirm_delete(state, key),
        // Fall through to the panel and global keys below.
        Mode::Browsing => {}
    }

    // ---- Global keys ----
    if is_quit(&key) {
        return Action::Quit;
    }

    // Ctrl+\ — toggle sidebar visibility
    if key.code == KeyCode::Char('\\') && key.modifiers == KeyModifiers::CONTROL {
        return Action::Nav(NavAction::ToggleSidebar);
    }

    // Ctrl+E — the rest of the message in the status bar. The bar is one row
    // wide, so a database error longer than the terminal used to be cut off
    // with no way to read the end of it.
    if key.code == KeyCode::Char('e') && key.modifiers == KeyModifiers::CONTROL {
        return Action::ShowNoticeDetail;
    }

    // Ctrl+hjkl moves between panels from anywhere — including mid-word in the
    // editor, which is the point: reaching the results used to be Esc, then a
    // direction, then Enter, and the first of those threw away the mode you
    // were in.
    //
    // Placed above the insert-mode passthrough below, or typing would swallow
    // it. On a terminal without the enhanced keyboard protocol, Ctrl+h and
    // Ctrl+j never arrive as themselves — they arrive as Backspace and Enter,
    // which keep working normally, so this costs those terminals nothing and
    // simply does not fire. Ctrl+k and Ctrl+l work everywhere.
    // Alt is accepted as well as Ctrl, and means exactly the same thing. Two
    // chords rather than one because whichever a user's compositor, terminal
    // or multiplexer has already claimed, the other is still theirs — and
    // until now Alt+hjkl was advertised as panel navigation while being dead
    // in the editor, which is where it is most wanted.
    if (key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT))
        && matches!(key.code, KeyCode::Char('h' | 'j' | 'k' | 'l'))
    {
        // Landing in a panel means working in it, so this enters panel mode
        // too — otherwise every arrival needed an Enter to be useful.
        return match navigation::try_navigate_panels(state, key) {
            Some(focus) => Action::Batch(vec![
                focus,
                Action::Nav(NavAction::SetNavMode(NavMode::Panel)),
            ]),
            // No neighbour that way. Swallowed rather than passed on, so a
            // panel key is never mistaken for a movement that did not happen.
            None => Action::Noop,
        };
    }

    // In Editor Insert mode, keep typing local to editor.
    if state.editor.mode == EditorMode::Insert && state.focused == FocusedPanel::Editor {
        if key.code == KeyCode::Esc {
            return Action::Batch(vec![
                Action::Nav(NavAction::SetEditorMode(EditorMode::Normal)),
                Action::Nav(NavAction::SetNavMode(NavMode::Panel)),
                Action::Nav(NavAction::SetPendingLeader(false)),
            ]);
        }
        return editor::handle(state, key);
    }

    // Esc leaves panel mode and returns to global mode.
    if key.code == KeyCode::Esc {
        let mut actions = vec![
            Action::Nav(NavAction::SetPendingLeader(false)),
            Action::Nav(NavAction::SetEditorMode(EditorMode::Normal)),
            // Failures do not time out, so Esc is how they go. Without this
            // the only way to clear one was to do something unrelated that
            // happened to overwrite it.
            Action::DismissNotice,
        ];
        if state.vim.nav_mode == NavMode::Panel {
            actions.push(Action::Nav(NavAction::SetNavMode(NavMode::Global)));
            actions.push(Action::Inform("Global mode".into()));
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
        return Action::Nav(NavAction::FocusPanel(target));
    }

    // Shift+D = open database diagram
    if key.code == KeyCode::Char('D') {
        return Action::Diagram(DiagramAction::Open);
    }

    // Tab / BackTab cycles focus
    if key.code == KeyCode::Tab && key.modifiers == KeyModifiers::NONE {
        return Action::Nav(NavAction::FocusPanel(navigation::tab_next(
            state.focused,
            state.layout.sidebar_hidden,
        )));
    }
    if key.code == KeyCode::BackTab {
        return Action::Nav(NavAction::FocusPanel(navigation::tab_prev(
            state.focused,
            state.layout.sidebar_hidden,
        )));
    }

    if state.vim.nav_mode == NavMode::Global {
        if state.vim.pending_leader {
            return match (key.code, key.modifiers) {
                (KeyCode::Char('e'), KeyModifiers::NONE) => Action::Batch(vec![
                    Action::Nav(NavAction::SetPendingLeader(false)),
                    Action::Nav(NavAction::ToggleSidebar),
                ]),
                _ => Action::Batch(vec![
                    Action::Nav(NavAction::SetPendingLeader(false)),
                    Action::Inform("Unknown leader combo. Try: Space e".into()),
                ]),
            };
        }

        if key.code == KeyCode::Char(' ') && key.modifiers == KeyModifiers::NONE {
            return Action::Batch(vec![
                Action::Nav(NavAction::SetPendingLeader(true)),
                Action::Inform("Leader: _  (e: toggle sidebar)".into()),
            ]);
        }

        if key.code == KeyCode::Char('i') && key.modifiers == KeyModifiers::NONE {
            return match state.focused {
                FocusedPanel::Results => Action::Batch(vec![
                    Action::Nav(NavAction::SetNavMode(NavMode::Panel)),
                    Action::CellEdit(CellEditAction::Enter),
                ]),
                FocusedPanel::Editor => Action::Batch(vec![
                    Action::Nav(NavAction::SetNavMode(NavMode::Panel)),
                    Action::Nav(NavAction::SetEditorMode(EditorMode::Insert)),
                ]),
                FocusedPanel::Connections | FocusedPanel::Tables => Action::Batch(vec![
                    Action::Nav(NavAction::FocusPanel(FocusedPanel::Editor)),
                    Action::Nav(NavAction::SetNavMode(NavMode::Panel)),
                    Action::Nav(NavAction::SetEditorMode(EditorMode::Insert)),
                ]),
            };
        }

        if key.code == KeyCode::Enter {
            return Action::Batch(vec![
                Action::Nav(NavAction::SetNavMode(NavMode::Panel)),
                Action::Inform("Panel mode".into()),
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
        return Action::Nav(NavAction::SetEditorMode(EditorMode::Insert));
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
    use crate::app::Mode;

    /// Opening one overlay must close any other, so `mode()` never has to
    /// arbitrate between two things that both think they own the keyboard.
    #[test]
    fn overlays_are_mutually_exclusive() {
        use crate::action::{apply, Action, ConnectionsAction, FilterAction};

        let mut state = crate::test_helpers::make_state_with_results();
        state.conn.connections = vec![sbql_core::ConnectionConfig::new_sqlite("c", "/tmp/x.db")];
        let (tx, _rx) = crate::test_helpers::cmd_channel();

        let opens: Vec<Action> = vec![
            Action::Filter(FilterAction::Open),
            Action::Connections(ConnectionsAction::OpenNewForm),
            Action::Connections(ConnectionsAction::InitDelete),
            Action::Connections(ConnectionsAction::OpenEditForm),
        ];
        for open in opens {
            apply(open, &mut state, &tx);
            assert_eq!(
                state.open_overlay_count(),
                1,
                "exactly one overlay may be open, mode is {:?}",
                state.mode()
            );
        }
    }

    /// The diagram sits above everything: while it is open no other overlay
    /// can steal a key press.
    #[test]
    fn mode_resolves_in_precedence_order() {
        let mut state = crate::test_helpers::make_state_with_results();
        assert_eq!(state.mode(), Mode::Browsing);

        state.conn.pending_delete = Some((uuid::Uuid::new_v4(), "c".into()));
        assert_eq!(state.mode(), Mode::ConfirmDelete);

        state.conn.form.visible = true;
        assert_eq!(state.mode(), Mode::ConnectionForm, "form outranks confirm");

        state.filter.visible = true;
        assert_eq!(state.mode(), Mode::Filter, "filter outranks form");

        state.diagram = Some(crate::app::DiagramState::new(
            sbql_core::DiagramData::default(),
        ));
        assert_eq!(state.mode(), Mode::Diagram, "diagram outranks everything");

        state.close_overlays();
        assert_eq!(state.mode(), Mode::Browsing);
        assert_eq!(state.open_overlay_count(), 0);
    }

    use super::*;
    use crate::action::{ConnectionsAction, FilterAction, FormAction};
    use crate::app::{CellEditState, DiagramState};
    use crate::test_helpers::{key, key_mod, make_state_with_results};
    use sbql_core::DiagramData;

    // -- Priority: diagram intercepts all --

    #[test]
    fn diagram_mode_intercepts() {
        let mut state = make_state_with_results();
        state.diagram = Some(DiagramState::new(DiagramData::default()));
        let act = handle_key(&state, key(KeyCode::Char('q')));
        assert!(matches!(act, Action::Diagram(DiagramAction::Close)));
    }

    /// The whole point of the binding: one chord out of the editor, without
    /// first leaving insert mode. This used to take Esc, a direction and
    /// Enter, and the Esc threw away the mode you were in.
    #[test]
    fn ctrl_hjkl_leaves_the_editor_mid_typing() {
        let mut state = make_state_with_results();
        state.focused = FocusedPanel::Editor;
        state.editor.mode = EditorMode::Insert;
        state.vim.nav_mode = NavMode::Panel;

        let act = handle_key(&state, key_mod(KeyCode::Char('j'), KeyModifiers::CONTROL));

        assert!(
            matches!(&act, Action::Batch(a) if a.iter().any(|x| matches!(
                x, Action::Nav(NavAction::FocusPanel(FocusedPanel::Results))
            ))),
            "{act:?}"
        );
    }

    /// Arriving in a panel should leave you able to work in it, or the chord
    /// has only replaced one of the three keys it was meant to replace.
    #[test]
    fn ctrl_hjkl_arrives_in_panel_mode() {
        let mut state = make_state_with_results();
        state.focused = FocusedPanel::Editor;
        state.vim.nav_mode = NavMode::Global;

        let act = handle_key(&state, key_mod(KeyCode::Char('j'), KeyModifiers::CONTROL));

        assert!(
            matches!(&act, Action::Batch(a) if a.iter().any(|x| matches!(
                x, Action::Nav(NavAction::SetNavMode(NavMode::Panel))
            ))),
            "{act:?}"
        );
    }

    /// Moving where there is no panel must do nothing at all, rather than
    /// falling through to whatever the focused panel does with that key.
    #[test]
    fn ctrl_hjkl_into_nothing_is_swallowed() {
        let mut state = make_state_with_results();
        state.focused = FocusedPanel::Results;

        // Nothing is to the right of the results.
        let act = handle_key(&state, key_mod(KeyCode::Char('l'), KeyModifiers::CONTROL));

        assert!(matches!(act, Action::Noop), "{act:?}");
    }

    /// Alt is the way out when something upstream has taken Ctrl. It has to
    /// reach as far as Ctrl does, including mid-word in the editor, or it is
    /// not an alternative at all.
    #[test]
    fn alt_hjkl_works_where_ctrl_does() {
        let mut state = make_state_with_results();
        state.focused = FocusedPanel::Editor;
        state.editor.mode = EditorMode::Insert;
        state.vim.nav_mode = NavMode::Panel;

        let act = handle_key(&state, key_mod(KeyCode::Char('j'), KeyModifiers::ALT));

        assert!(
            matches!(&act, Action::Batch(a) if a.iter().any(|x| matches!(
                x, Action::Nav(NavAction::FocusPanel(FocusedPanel::Results))
            ))),
            "{act:?}"
        );
    }

    /// The diagram is a full-screen overlay with no panels, and it uses
    /// Ctrl+h/l for fast horizontal scrolling. It has to keep them.
    #[test]
    fn the_diagram_keeps_its_own_ctrl_h() {
        let mut state = make_state_with_results();
        state.diagram = Some(crate::app::DiagramState::new(Default::default()));

        let act = handle_key(&state, key_mod(KeyCode::Char('h'), KeyModifiers::CONTROL));

        assert!(
            matches!(
                act,
                Action::Diagram(crate::action::DiagramAction::Scroll { .. })
            ),
            "{act:?}"
        );
    }

    // -- Priority: cell edit --

    #[test]
    fn cell_edit_mode_intercepts() {
        let mut state = make_state_with_results();
        state.mutation.cell_edit = Some(CellEditState::new(
            0,
            0,
            "id".into(),
            "1".into(),
            "public".into(),
            "users".into(),
            vec![("id".into(), "1".into())],
        ));
        let act = handle_key(&state, key(KeyCode::Esc));
        assert!(matches!(act, Action::CellEdit(CellEditAction::Cancel)));
    }

    // -- Priority: filter --

    #[test]
    fn filter_mode_intercepts() {
        let mut state = make_state_with_results();
        state.filter.visible = true;
        let act = handle_key(&state, key(KeyCode::Esc));
        assert!(matches!(act, Action::Filter(FilterAction::Close)));
    }

    // -- Priority: connection form --

    #[test]
    fn form_mode_intercepts() {
        let mut state = make_state_with_results();
        state.conn.form.visible = true;
        let act = handle_key(&state, key(KeyCode::Esc));
        assert!(matches!(act, Action::Form(FormAction::Close)));
    }

    // -- Priority: delete confirm --

    #[test]
    fn delete_confirm_intercepts() {
        let mut state = make_state_with_results();
        state.conn.pending_delete = Some((uuid::Uuid::new_v4(), "test".into()));
        let act = handle_key(&state, key(KeyCode::Char('y')));
        assert!(matches!(
            act,
            Action::Connections(ConnectionsAction::ConfirmDelete)
        ));
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
        assert!(matches!(act, Action::Nav(NavAction::ToggleSidebar)));
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
        assert!(matches!(
            act,
            Action::Nav(NavAction::FocusPanel(FocusedPanel::Connections))
        ));
    }

    #[test]
    fn f2_focuses_tables() {
        let state = make_state_with_results();
        let act = handle_key(&state, key(KeyCode::F(2)));
        assert!(matches!(
            act,
            Action::Nav(NavAction::FocusPanel(FocusedPanel::Tables))
        ));
    }

    #[test]
    fn f3_focuses_editor() {
        let state = make_state_with_results();
        let act = handle_key(&state, key(KeyCode::F(3)));
        assert!(matches!(
            act,
            Action::Nav(NavAction::FocusPanel(FocusedPanel::Editor))
        ));
    }

    #[test]
    fn f4_focuses_results() {
        let state = make_state_with_results();
        let act = handle_key(&state, key(KeyCode::F(4)));
        assert!(matches!(
            act,
            Action::Nav(NavAction::FocusPanel(FocusedPanel::Results))
        ));
    }

    // -- Tab cycles --

    #[test]
    fn tab_cycles_forward() {
        let mut state = make_state_with_results();
        state.focused = FocusedPanel::Connections;
        let act = handle_key(&state, key(KeyCode::Tab));
        assert!(matches!(
            act,
            Action::Nav(NavAction::FocusPanel(FocusedPanel::Tables))
        ));
    }

    #[test]
    fn backtab_cycles_backward() {
        let mut state = make_state_with_results();
        state.focused = FocusedPanel::Tables;
        let act = handle_key(&state, key(KeyCode::BackTab));
        assert!(matches!(
            act,
            Action::Nav(NavAction::FocusPanel(FocusedPanel::Connections))
        ));
    }

    // -- Shift+D opens diagram --

    #[test]
    fn shift_d_opens_diagram() {
        let state = make_state_with_results();
        let act = handle_key(&state, key(KeyCode::Char('D')));
        assert!(matches!(act, Action::Diagram(DiagramAction::Open)));
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
        // A batch now: arriving in a panel also enters panel mode, so the
        // panel is usable without a further keypress.
        assert!(
            matches!(&act, Action::Batch(a) if a.iter().any(|x| matches!(
                x, Action::Nav(NavAction::FocusPanel(FocusedPanel::Editor))
            ))),
            "{act:?}"
        );
    }

    // -- Panel mode: i enters insert in editor --

    #[test]
    fn i_panel_mode_editor_normal_enters_insert() {
        let mut state = make_state_with_results();
        state.vim.nav_mode = NavMode::Panel;
        state.focused = FocusedPanel::Editor;
        state.editor.mode = EditorMode::Normal;
        let act = handle_key(&state, key(KeyCode::Char('i')));
        assert!(matches!(
            act,
            Action::Nav(NavAction::SetEditorMode(EditorMode::Insert))
        ));
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
