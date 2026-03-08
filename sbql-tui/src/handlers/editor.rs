use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_textarea::{CursorMove, Input};

use crate::action::Action;
use crate::app::{AppState, EditorMode};
use crate::events::is_run_query;

pub fn handle(state: &AppState, key: KeyEvent) -> Action {
    match state.editor.mode {
        EditorMode::Normal => match (key.code, key.modifiers) {
            (KeyCode::Char('i'), KeyModifiers::NONE) => {
                Action::SetEditorMode(EditorMode::Insert)
            }
            (KeyCode::Char('h') | KeyCode::Left, KeyModifiers::NONE) => {
                Action::EditorCursorMove(CursorMove::Back)
            }
            (KeyCode::Char('l') | KeyCode::Right, KeyModifiers::NONE) => {
                Action::EditorCursorMove(CursorMove::Forward)
            }
            (KeyCode::Char('j') | KeyCode::Down, KeyModifiers::NONE) => {
                Action::EditorCursorMove(CursorMove::Down)
            }
            (KeyCode::Char('k') | KeyCode::Up, KeyModifiers::NONE) => {
                Action::EditorCursorMove(CursorMove::Up)
            }
            (KeyCode::Char('w'), KeyModifiers::NONE) => {
                Action::EditorCursorMove(CursorMove::WordForward)
            }
            (KeyCode::Char('b'), KeyModifiers::NONE) => {
                Action::EditorCursorMove(CursorMove::WordBack)
            }
            (KeyCode::Char('0'), KeyModifiers::NONE) => {
                Action::EditorCursorMove(CursorMove::Head)
            }
            (KeyCode::Char('$'), KeyModifiers::NONE) => {
                Action::EditorCursorMove(CursorMove::End)
            }
            (KeyCode::Char('g'), KeyModifiers::NONE) => {
                Action::EditorCursorMove(CursorMove::Top)
            }
            (KeyCode::Char('G'), _) => Action::EditorCursorMove(CursorMove::Bottom),
            _ if is_run_query(&key) => Action::RunQuery,
            (KeyCode::Esc, _) => Action::Noop,
            _ => Action::Noop,
        },
        EditorMode::Insert => {
            if is_run_query(&key) {
                return Action::RunQuery;
            }
            if state.editor.completion.visible {
                match key.code {
                    KeyCode::Up => return Action::CompletionUp,
                    KeyCode::Down => return Action::CompletionDown,
                    KeyCode::Tab | KeyCode::Enter => return Action::CompletionAccept,
                    KeyCode::Esc => {
                        return Action::Batch(vec![
                            Action::CompletionDismiss,
                            Action::SetEditorMode(EditorMode::Normal),
                        ]);
                    }
                    _ => {} // fall through to normal editor input
                }
            } else if key.code == KeyCode::Esc {
                return Action::SetEditorMode(EditorMode::Normal);
            }
            Action::EditorInput(Input::from(key))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{key, key_mod};
    use crossterm::event::KeyModifiers;

    fn normal_state() -> AppState {
        let mut state = AppState::new(vec![]);
        state.editor.mode = EditorMode::Normal;
        state
    }

    fn insert_state() -> AppState {
        let mut state = AppState::new(vec![]);
        state.editor.mode = EditorMode::Insert;
        state
    }

    // -- Normal mode --

    #[test]
    fn normal_i_enters_insert() {
        let state = normal_state();
        let act = handle(&state, key(KeyCode::Char('i')));
        assert!(matches!(act, Action::SetEditorMode(EditorMode::Insert)));
    }

    #[test]
    fn normal_h_moves_back() {
        let state = normal_state();
        let act = handle(&state, key(KeyCode::Char('h')));
        assert!(matches!(act, Action::EditorCursorMove(CursorMove::Back)));
    }

    #[test]
    fn normal_l_moves_forward() {
        let state = normal_state();
        let act = handle(&state, key(KeyCode::Char('l')));
        assert!(matches!(act, Action::EditorCursorMove(CursorMove::Forward)));
    }

    #[test]
    fn normal_j_moves_down() {
        let state = normal_state();
        let act = handle(&state, key(KeyCode::Char('j')));
        assert!(matches!(act, Action::EditorCursorMove(CursorMove::Down)));
    }

    #[test]
    fn normal_k_moves_up() {
        let state = normal_state();
        let act = handle(&state, key(KeyCode::Char('k')));
        assert!(matches!(act, Action::EditorCursorMove(CursorMove::Up)));
    }

    #[test]
    fn normal_w_word_forward() {
        let state = normal_state();
        let act = handle(&state, key(KeyCode::Char('w')));
        assert!(matches!(act, Action::EditorCursorMove(CursorMove::WordForward)));
    }

    #[test]
    fn normal_b_word_back() {
        let state = normal_state();
        let act = handle(&state, key(KeyCode::Char('b')));
        assert!(matches!(act, Action::EditorCursorMove(CursorMove::WordBack)));
    }

    #[test]
    fn normal_0_head() {
        let state = normal_state();
        let act = handle(&state, key(KeyCode::Char('0')));
        assert!(matches!(act, Action::EditorCursorMove(CursorMove::Head)));
    }

    #[test]
    fn normal_dollar_end() {
        let state = normal_state();
        let act = handle(&state, key(KeyCode::Char('$')));
        assert!(matches!(act, Action::EditorCursorMove(CursorMove::End)));
    }

    #[test]
    fn normal_g_top() {
        let state = normal_state();
        let act = handle(&state, key(KeyCode::Char('g')));
        assert!(matches!(act, Action::EditorCursorMove(CursorMove::Top)));
    }

    #[test]
    fn normal_shift_g_bottom() {
        let state = normal_state();
        let act = handle(&state, key(KeyCode::Char('G')));
        assert!(matches!(act, Action::EditorCursorMove(CursorMove::Bottom)));
    }

    #[test]
    fn normal_ctrl_s_runs_query() {
        let state = normal_state();
        let act = handle(&state, key_mod(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert!(matches!(act, Action::RunQuery));
    }

    #[test]
    fn normal_f5_runs_query() {
        let state = normal_state();
        let act = handle(&state, key(KeyCode::F(5)));
        assert!(matches!(act, Action::RunQuery));
    }

    // -- Insert mode --

    #[test]
    fn insert_esc_returns_normal() {
        let state = insert_state();
        let act = handle(&state, key(KeyCode::Esc));
        assert!(matches!(act, Action::SetEditorMode(EditorMode::Normal)));
    }

    #[test]
    fn insert_ctrl_s_runs_query() {
        let state = insert_state();
        let act = handle(&state, key_mod(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert!(matches!(act, Action::RunQuery));
    }

    #[test]
    fn insert_f5_runs_query() {
        let state = insert_state();
        let act = handle(&state, key(KeyCode::F(5)));
        assert!(matches!(act, Action::RunQuery));
    }

    #[test]
    fn insert_char_produces_editor_input() {
        let state = insert_state();
        let act = handle(&state, key(KeyCode::Char('x')));
        assert!(matches!(act, Action::EditorInput(_)));
    }
}
