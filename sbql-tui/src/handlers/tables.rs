use crossterm::event::{KeyCode, KeyEvent};

use crate::action::{Action, NavAction, TablesAction};
use crate::app::{AppState, FocusedPanel};

pub fn handle(state: &AppState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            if !state.tables.tables.is_empty() {
                let next = state.tables.selected() + 1;
                Action::Batch(vec![
                    Action::Nav(NavAction::ClearPendingG),
                    Action::Tables(TablesAction::Select(next)),
                ])
            } else {
                Action::Nav(NavAction::ClearPendingG)
            }
        }
        KeyCode::Up | KeyCode::Char('k') => Action::Batch(vec![
            Action::Nav(NavAction::ClearPendingG),
            Action::Tables(TablesAction::Select(
                state.tables.selected().saturating_sub(1),
            )),
        ]),
        KeyCode::Char('G') => {
            if !state.tables.tables.is_empty() {
                Action::Batch(vec![
                    Action::Nav(NavAction::ClearPendingG),
                    Action::Tables(TablesAction::Select(state.tables.tables.len() - 1)),
                ])
            } else {
                Action::Nav(NavAction::ClearPendingG)
            }
        }
        KeyCode::Char('g') => {
            if state.vim.pending_g {
                Action::Batch(vec![
                    Action::Nav(NavAction::ClearPendingG),
                    Action::Tables(TablesAction::Select(0)),
                ])
            } else {
                Action::Nav(NavAction::SetPendingG)
            }
        }
        KeyCode::Enter => Action::Batch(vec![
            Action::Nav(NavAction::ClearPendingG),
            Action::Tables(TablesAction::OpenSelected),
        ]),
        KeyCode::Esc => Action::Batch(vec![
            Action::Nav(NavAction::ClearPendingG),
            Action::Nav(NavAction::FocusPanel(FocusedPanel::Editor)),
        ]),
        _ => Action::Nav(NavAction::ClearPendingG),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::key;
    use sbql_core::TableEntry;

    fn state_with_tables(n: usize) -> AppState {
        let mut state = AppState::new(vec![]);
        state.tables.tables = (0..n)
            .map(|i| TableEntry {
                schema: "public".into(),
                name: format!("table_{i}"),
            })
            .collect();
        state
    }

    #[test]
    fn j_moves_down() {
        let state = state_with_tables(3);
        let act = handle(&state, key(KeyCode::Char('j')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn j_empty_clears_pending() {
        let state = state_with_tables(0);
        let act = handle(&state, key(KeyCode::Char('j')));
        assert!(matches!(act, Action::Nav(NavAction::ClearPendingG)));
    }

    #[test]
    fn k_moves_up() {
        let mut state = state_with_tables(3);
        state.tables.cursor.select(2, state.tables.tables.len());
        let act = handle(&state, key(KeyCode::Char('k')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn shift_g_jumps_to_last() {
        let state = state_with_tables(5);
        let act = handle(&state, key(KeyCode::Char('G')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn gg_jumps_to_first() {
        let mut state = state_with_tables(5);
        state.vim.pending_g = true;
        let act = handle(&state, key(KeyCode::Char('g')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn g_sets_pending() {
        let state = state_with_tables(3);
        let act = handle(&state, key(KeyCode::Char('g')));
        assert!(matches!(act, Action::Nav(NavAction::SetPendingG)));
    }

    #[test]
    fn enter_opens_table() {
        let state = state_with_tables(1);
        let act = handle(&state, key(KeyCode::Enter));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn esc_focuses_editor() {
        let state = state_with_tables(1);
        let act = handle(&state, key(KeyCode::Esc));
        assert!(matches!(act, Action::Batch(_)));
    }
}
