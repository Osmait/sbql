use crossterm::event::{KeyCode, KeyEvent};

use crate::action::Action;
use crate::app::{AppState, FocusedPanel};

pub fn handle(state: &AppState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            if !state.conn.connections.is_empty() {
                let next = (state.conn.selected + 1).min(state.conn.connections.len() - 1);
                Action::Batch(vec![Action::ClearPendingG, Action::SelectConnection(next)])
            } else {
                Action::ClearPendingG
            }
        }
        KeyCode::Up | KeyCode::Char('k') => Action::Batch(vec![
            Action::ClearPendingG,
            Action::SelectConnection(state.conn.selected.saturating_sub(1)),
        ]),
        KeyCode::Char('G') => {
            if !state.conn.connections.is_empty() {
                Action::Batch(vec![
                    Action::ClearPendingG,
                    Action::SelectConnection(state.conn.connections.len() - 1),
                ])
            } else {
                Action::ClearPendingG
            }
        }
        KeyCode::Char('g') => {
            if state.vim.pending_g {
                Action::Batch(vec![Action::ClearPendingG, Action::SelectConnection(0)])
            } else {
                Action::SetPendingG
            }
        }
        KeyCode::Enter => Action::Batch(vec![Action::ClearPendingG, Action::ConnectSelected]),
        KeyCode::Char('n') => Action::Batch(vec![Action::ClearPendingG, Action::OpenNewConnForm]),
        KeyCode::Char('e') => Action::Batch(vec![Action::ClearPendingG, Action::OpenEditConnForm]),
        KeyCode::Char('d') => {
            Action::Batch(vec![Action::ClearPendingG, Action::InitDeleteConnection])
        }
        KeyCode::Char('x') => Action::Batch(vec![Action::ClearPendingG, Action::DisconnectActive]),
        KeyCode::Esc => Action::Batch(vec![
            Action::ClearPendingG,
            Action::FocusPanel(FocusedPanel::Editor),
        ]),
        _ => Action::ClearPendingG,
    }
}

pub fn handle_confirm_delete(_state: &AppState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => Action::ConfirmDeleteConnection,
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Action::CancelDeleteConnection,
        _ => Action::Noop,
    }
}

pub fn handle_form(state: &AppState, key: KeyEvent) -> Action {
    let form = &state.conn.form;
    match key.code {
        KeyCode::Esc => Action::FormClose,
        KeyCode::Tab | KeyCode::Down => Action::FormNextField,
        KeyCode::BackTab | KeyCode::Up => Action::FormPrevField,
        KeyCode::Enter => Action::FormSubmit,
        KeyCode::Char(' ') if form.field_index == 0 => Action::FormCycleBackend,
        KeyCode::Char(' ')
            if form.backend == sbql_core::DbBackend::Postgres && form.field_index == 7 =>
        {
            Action::FormCycleSsl
        }
        KeyCode::Backspace => Action::FormBackspace,
        KeyCode::Char(c) => Action::FormInput(c),
        _ => Action::Noop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::key;
    use sbql_core::ConnectionConfig;

    fn state_with_conns(n: usize) -> AppState {
        let conns: Vec<ConnectionConfig> = (0..n)
            .map(|i| ConnectionConfig::new(format!("c{i}"), "host", 5432, "user", "db"))
            .collect();
        AppState::new(conns)
    }

    // -- handle --

    #[test]
    fn handle_j_moves_down() {
        let state = state_with_conns(3);
        let act = handle(&state, key(KeyCode::Char('j')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn handle_k_moves_up() {
        let mut state = state_with_conns(3);
        state.conn.selected = 2;
        let act = handle(&state, key(KeyCode::Char('k')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn handle_j_empty_list_clears_pending() {
        let state = state_with_conns(0);
        let act = handle(&state, key(KeyCode::Char('j')));
        assert!(matches!(act, Action::ClearPendingG));
    }

    #[test]
    fn handle_shift_g_jumps_to_last() {
        let state = state_with_conns(5);
        let act = handle(&state, key(KeyCode::Char('G')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn handle_g_without_pending_sets_pending() {
        let state = state_with_conns(3);
        let act = handle(&state, key(KeyCode::Char('g')));
        assert!(matches!(act, Action::SetPendingG));
    }

    #[test]
    fn handle_gg_jumps_to_first() {
        let mut state = state_with_conns(3);
        state.vim.pending_g = true;
        let act = handle(&state, key(KeyCode::Char('g')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn handle_enter_connects() {
        let state = state_with_conns(1);
        let act = handle(&state, key(KeyCode::Enter));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn handle_n_opens_new_form() {
        let state = state_with_conns(0);
        let act = handle(&state, key(KeyCode::Char('n')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn handle_e_opens_edit_form() {
        let state = state_with_conns(1);
        let act = handle(&state, key(KeyCode::Char('e')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn handle_d_inits_delete() {
        let state = state_with_conns(1);
        let act = handle(&state, key(KeyCode::Char('d')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn handle_x_disconnects() {
        let state = state_with_conns(1);
        let act = handle(&state, key(KeyCode::Char('x')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn handle_esc_focuses_editor() {
        let state = state_with_conns(1);
        let act = handle(&state, key(KeyCode::Esc));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn handle_unknown_clears_pending() {
        let state = state_with_conns(1);
        let act = handle(&state, key(KeyCode::Char('z')));
        assert!(matches!(act, Action::ClearPendingG));
    }

    // -- handle_confirm_delete --

    #[test]
    fn confirm_delete_y() {
        let state = state_with_conns(0);
        let act = handle_confirm_delete(&state, key(KeyCode::Char('y')));
        assert!(matches!(act, Action::ConfirmDeleteConnection));
    }

    #[test]
    fn confirm_delete_enter() {
        let state = state_with_conns(0);
        let act = handle_confirm_delete(&state, key(KeyCode::Enter));
        assert!(matches!(act, Action::ConfirmDeleteConnection));
    }

    #[test]
    fn cancel_delete_n() {
        let state = state_with_conns(0);
        let act = handle_confirm_delete(&state, key(KeyCode::Char('n')));
        assert!(matches!(act, Action::CancelDeleteConnection));
    }

    #[test]
    fn cancel_delete_esc() {
        let state = state_with_conns(0);
        let act = handle_confirm_delete(&state, key(KeyCode::Esc));
        assert!(matches!(act, Action::CancelDeleteConnection));
    }

    #[test]
    fn confirm_delete_unknown_noop() {
        let state = state_with_conns(0);
        let act = handle_confirm_delete(&state, key(KeyCode::Char('z')));
        assert!(matches!(act, Action::Noop));
    }

    // -- handle_form --

    #[test]
    fn form_esc_closes() {
        let mut state = state_with_conns(0);
        state.conn.form.visible = true;
        let act = handle_form(&state, key(KeyCode::Esc));
        assert!(matches!(act, Action::FormClose));
    }

    #[test]
    fn form_tab_next_field() {
        let mut state = state_with_conns(0);
        state.conn.form.visible = true;
        let act = handle_form(&state, key(KeyCode::Tab));
        assert!(matches!(act, Action::FormNextField));
    }

    #[test]
    fn form_backtab_prev_field() {
        let mut state = state_with_conns(0);
        state.conn.form.visible = true;
        let act = handle_form(&state, key(KeyCode::BackTab));
        assert!(matches!(act, Action::FormPrevField));
    }

    #[test]
    fn form_space_on_backend_field_cycles() {
        let mut state = state_with_conns(0);
        state.conn.form.visible = true;
        state.conn.form.field_index = 0;
        let act = handle_form(&state, key(KeyCode::Char(' ')));
        assert!(matches!(act, Action::FormCycleBackend));
    }

    #[test]
    fn form_space_on_ssl_field_cycles() {
        let mut state = state_with_conns(0);
        state.conn.form.visible = true;
        state.conn.form.field_index = 7;
        let act = handle_form(&state, key(KeyCode::Char(' ')));
        assert!(matches!(act, Action::FormCycleSsl));
    }

    #[test]
    fn form_enter_submits() {
        let mut state = state_with_conns(0);
        state.conn.form.visible = true;
        let act = handle_form(&state, key(KeyCode::Enter));
        assert!(matches!(act, Action::FormSubmit));
    }

    #[test]
    fn form_char_input() {
        let mut state = state_with_conns(0);
        state.conn.form.visible = true;
        let act = handle_form(&state, key(KeyCode::Char('a')));
        assert!(matches!(act, Action::FormInput('a')));
    }

    #[test]
    fn form_backspace() {
        let mut state = state_with_conns(0);
        state.conn.form.visible = true;
        let act = handle_form(&state, key(KeyCode::Backspace));
        assert!(matches!(act, Action::FormBackspace));
    }
}
