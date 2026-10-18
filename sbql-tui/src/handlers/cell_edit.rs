use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_textarea::Input;

use crate::action::{Action, CellEditAction, ResultsAction};
use crate::app::AppState;
use crate::events::is_commit;

pub(crate) fn handle(_state: &AppState, key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => Action::CellEdit(CellEditAction::Cancel),
        (KeyCode::Char('s'), KeyModifiers::CONTROL) | (KeyCode::Enter, KeyModifiers::NONE) => {
            Action::CellEdit(CellEditAction::Stage)
        }
        _ if is_commit(&key) => Action::Batch(vec![
            Action::CellEdit(CellEditAction::Stage),
            Action::Results(ResultsAction::CommitPending),
        ]),
        _ => Action::CellEdit(CellEditAction::Input(Input::from(key))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{key, key_mod};
    use crossterm::event::KeyModifiers;

    fn state() -> AppState {
        AppState::new(vec![])
    }

    #[test]
    fn esc_cancels() {
        let s = state();
        let act = handle(&s, key(KeyCode::Esc));
        assert!(matches!(act, Action::CellEdit(CellEditAction::Cancel)));
    }

    #[test]
    fn ctrl_s_stages() {
        let s = state();
        let act = handle(&s, key_mod(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert!(matches!(act, Action::CellEdit(CellEditAction::Stage)));
    }

    #[test]
    fn enter_stages() {
        let s = state();
        let act = handle(&s, key(KeyCode::Enter));
        assert!(matches!(act, Action::CellEdit(CellEditAction::Stage)));
    }

    #[test]
    fn ctrl_w_stage_and_commit() {
        let s = state();
        let act = handle(&s, key_mod(KeyCode::Char('w'), KeyModifiers::CONTROL));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn char_produces_input() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('x')));
        assert!(matches!(act, Action::CellEdit(CellEditAction::Input(_))));
    }
}
