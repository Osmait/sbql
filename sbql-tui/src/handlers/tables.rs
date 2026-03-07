use crossterm::event::{KeyCode, KeyEvent};

use crate::action::Action;
use crate::app::{AppState, FocusedPanel};

pub fn handle(state: &AppState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            if !state.tables.tables.is_empty() {
                let next = (state.tables.selected + 1).min(state.tables.tables.len() - 1);
                Action::Batch(vec![Action::ClearPendingG, Action::SelectTable(next)])
            } else {
                Action::ClearPendingG
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            Action::Batch(vec![
                Action::ClearPendingG,
                Action::SelectTable(state.tables.selected.saturating_sub(1)),
            ])
        }
        KeyCode::Char('G') => {
            if !state.tables.tables.is_empty() {
                Action::Batch(vec![
                    Action::ClearPendingG,
                    Action::SelectTable(state.tables.tables.len() - 1),
                ])
            } else {
                Action::ClearPendingG
            }
        }
        KeyCode::Char('g') => {
            if state.vim.pending_g {
                Action::Batch(vec![Action::ClearPendingG, Action::SelectTable(0)])
            } else {
                Action::SetPendingG
            }
        }
        KeyCode::Enter => {
            Action::Batch(vec![Action::ClearPendingG, Action::OpenSelectedTable])
        }
        KeyCode::Esc => {
            Action::Batch(vec![
                Action::ClearPendingG,
                Action::FocusPanel(FocusedPanel::Editor),
            ])
        }
        _ => Action::ClearPendingG,
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
        assert!(matches!(act, Action::ClearPendingG));
    }

    #[test]
    fn k_moves_up() {
        let mut state = state_with_tables(3);
        state.tables.selected = 2;
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
        assert!(matches!(act, Action::SetPendingG));
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
