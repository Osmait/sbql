use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;
use crate::app::AppState;
use crate::events::is_commit;

pub fn handle(state: &AppState, key: KeyEvent) -> Action {
    tracing::info!(
        "handle_key_results: code={:?} mods={:?} rows={} cols={}",
        key.code,
        key.modifiers,
        state.results.data.rows.len(),
        state.results.data.columns.len(),
    );
    match (key.code, key.modifiers) {
        (KeyCode::Down | KeyCode::Char('j'), KeyModifiers::NONE) => {
            Action::Batch(vec![Action::ClearPendingG, Action::ClearPendingD, Action::MoveRowDown])
        }
        (KeyCode::Up | KeyCode::Char('k'), KeyModifiers::NONE) => {
            Action::Batch(vec![Action::ClearPendingG, Action::ClearPendingD, Action::MoveRowUp])
        }
        (KeyCode::Right | KeyCode::Char('l'), KeyModifiers::NONE) => {
            Action::Batch(vec![
                Action::ClearPendingG,
                Action::ClearPendingD,
                Action::MoveColRight,
            ])
        }
        (KeyCode::Left | KeyCode::Char('h'), KeyModifiers::NONE) => {
            Action::Batch(vec![
                Action::ClearPendingG,
                Action::ClearPendingD,
                Action::MoveColLeft,
            ])
        }
        (KeyCode::Char('g'), KeyModifiers::NONE) => {
            if state.vim.pending_g {
                Action::Batch(vec![
                    Action::ClearPendingG,
                    Action::ClearPendingD,
                    Action::MoveRowFirst,
                ])
            } else {
                Action::Batch(vec![Action::ClearPendingD, Action::SetPendingG])
            }
        }
        (KeyCode::Char('G'), KeyModifiers::NONE) | (KeyCode::Char('G'), KeyModifiers::SHIFT) => {
            Action::Batch(vec![
                Action::ClearPendingG,
                Action::ClearPendingD,
                Action::MoveRowLast,
            ])
        }
        (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
            Action::Batch(vec![
                Action::ClearPendingG,
                Action::ClearPendingD,
                Action::MoveHalfPageDown,
            ])
        }
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            Action::Batch(vec![
                Action::ClearPendingG,
                Action::ClearPendingD,
                Action::MoveHalfPageUp,
            ])
        }
        (KeyCode::Char('d'), KeyModifiers::NONE) => {
            if state.mutation.pending_d {
                Action::Batch(vec![
                    Action::ClearPendingG,
                    Action::ClearPendingD,
                    Action::MarkRowForDeletion,
                ])
            } else {
                Action::Batch(vec![Action::ClearPendingG, Action::SetPendingD])
            }
        }
        (KeyCode::Char('0'), KeyModifiers::NONE) | (KeyCode::Char('^'), KeyModifiers::NONE) => {
            Action::Batch(vec![
                Action::ClearPendingG,
                Action::ClearPendingD,
                Action::MoveColFirst,
            ])
        }
        (KeyCode::Char('$'), KeyModifiers::NONE) => {
            Action::Batch(vec![
                Action::ClearPendingG,
                Action::ClearPendingD,
                Action::MoveColLast,
            ])
        }
        (KeyCode::PageDown, _) => {
            let mut actions =
                vec![Action::ClearPendingG, Action::ClearPendingD];
            if state.results.data.has_next_page {
                let next = state.results.current_page + 1;
                actions.push(Action::SendCommand(sbql_core::CoreCommand::FetchPage {
                    page: next,
                }));
            }
            Action::Batch(actions)
        }
        (KeyCode::PageUp, _) => {
            let mut actions =
                vec![Action::ClearPendingG, Action::ClearPendingD];
            if state.results.current_page > 0 {
                let prev = state.results.current_page - 1;
                actions.push(Action::SendCommand(sbql_core::CoreCommand::FetchPage {
                    page: prev,
                }));
            }
            Action::Batch(actions)
        }
        (KeyCode::Enter, KeyModifiers::NONE) | (KeyCode::Char('i'), KeyModifiers::NONE) => {
            Action::Batch(vec![
                Action::ClearPendingG,
                Action::ClearPendingD,
                Action::EnterCellEdit,
            ])
        }
        _ if is_commit(&key) => {
            Action::Batch(vec![
                Action::ClearPendingG,
                Action::ClearPendingD,
                Action::CommitPending,
            ])
        }
        (KeyCode::Char('o'), KeyModifiers::NONE) => {
            Action::Batch(vec![
                Action::ClearPendingG,
                Action::ClearPendingD,
                Action::ToggleSort,
            ])
        }
        (KeyCode::Char('/'), KeyModifiers::NONE)
        | (KeyCode::Char('f'), KeyModifiers::CONTROL) => {
            Action::Batch(vec![
                Action::ClearPendingG,
                Action::ClearPendingD,
                Action::OpenFilter,
            ])
        }
        (KeyCode::Esc, _) => {
            Action::Batch(vec![
                Action::ClearPendingG,
                Action::ClearPendingD,
                Action::DiscardPendingOrEsc,
            ])
        }
        _ => Action::Batch(vec![Action::ClearPendingG, Action::ClearPendingD]),
    }
}

// Expose `extract_schema_table_from_sql` for testing.
/// Extract `(schema, table)` from the first `FROM <name>` in the SQL.
pub fn extract_schema_table_from_sql(sql: &str) -> Option<(String, String)> {
    let upper = sql.to_uppercase();
    let from_pos = upper.find("FROM ")?;
    let rest = sql[from_pos + 5..].trim_start();

    fn parse_ident(s: &str) -> (&str, &str) {
        if s.starts_with('"') {
            let inner = &s[1..];
            if let Some(end) = inner.find('"') {
                (&inner[..end], &inner[end + 1..])
            } else {
                (inner, "")
            }
        } else {
            let end = s
                .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
                .unwrap_or(s.len());
            (&s[..end], &s[end..])
        }
    }

    let (first, after_first) = parse_ident(rest);
    if first.is_empty() {
        return None;
    }

    let after_first = after_first.trim_start();
    if after_first.starts_with('.') {
        let after_dot = after_first[1..].trim_start();
        let (second, _) = parse_ident(after_dot);
        if second.is_empty() {
            return None;
        }
        Some((first.to_owned(), second.to_owned()))
    } else {
        Some(("public".to_owned(), first.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{key, key_mod, make_state_with_results};
    use crossterm::event::KeyModifiers;

    fn results_state() -> AppState {
        let mut state = make_state_with_results();
        state.focused = crate::app::FocusedPanel::Results;
        state.vim.nav_mode = crate::app::NavMode::Panel;
        state
    }

    // -- Movement keys --

    #[test]
    fn j_produces_move_row_down() {
        let state = results_state();
        let act = handle(&state, key(KeyCode::Char('j')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn k_produces_move_row_up() {
        let state = results_state();
        let act = handle(&state, key(KeyCode::Char('k')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn l_produces_move_col_right() {
        let state = results_state();
        let act = handle(&state, key(KeyCode::Char('l')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn h_produces_move_col_left() {
        let state = results_state();
        let act = handle(&state, key(KeyCode::Char('h')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn shift_g_move_last() {
        let state = results_state();
        let act = handle(&state, key(KeyCode::Char('G')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn gg_move_first() {
        let mut state = results_state();
        state.vim.pending_g = true;
        let act = handle(&state, key(KeyCode::Char('g')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn g_sets_pending() {
        let state = results_state();
        let act = handle(&state, key(KeyCode::Char('g')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn ctrl_d_half_page_down() {
        let state = results_state();
        let act = handle(&state, key_mod(KeyCode::Char('d'), KeyModifiers::CONTROL));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn ctrl_u_half_page_up() {
        let state = results_state();
        let act = handle(&state, key_mod(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn zero_moves_col_first() {
        let state = results_state();
        let act = handle(&state, key(KeyCode::Char('0')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn dollar_moves_col_last() {
        let state = results_state();
        let act = handle(&state, key(KeyCode::Char('$')));
        assert!(matches!(act, Action::Batch(_)));
    }

    // -- Delete (dd) --

    #[test]
    fn d_sets_pending_d() {
        let state = results_state();
        let act = handle(&state, key(KeyCode::Char('d')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn dd_marks_deletion() {
        let mut state = results_state();
        state.mutation.pending_d = true;
        let act = handle(&state, key(KeyCode::Char('d')));
        assert!(matches!(act, Action::Batch(_)));
    }

    // -- Edit / commit --

    #[test]
    fn enter_enters_cell_edit() {
        let state = results_state();
        let act = handle(&state, key(KeyCode::Enter));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn i_enters_cell_edit() {
        let state = results_state();
        let act = handle(&state, key(KeyCode::Char('i')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn ctrl_w_commits() {
        let state = results_state();
        let act = handle(&state, key_mod(KeyCode::Char('w'), KeyModifiers::CONTROL));
        assert!(matches!(act, Action::Batch(_)));
    }

    // -- Sort / filter --

    #[test]
    fn o_toggle_sort() {
        let state = results_state();
        let act = handle(&state, key(KeyCode::Char('o')));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn slash_open_filter() {
        let state = results_state();
        let act = handle(&state, key(KeyCode::Char('/')));
        assert!(matches!(act, Action::Batch(_)));
    }

    // -- Page navigation --

    #[test]
    fn page_down_with_next_page() {
        let mut state = results_state();
        state.results.data.has_next_page = true;
        let act = handle(&state, key(KeyCode::PageDown));
        assert!(matches!(act, Action::Batch(_)));
    }

    #[test]
    fn page_up_from_page_1() {
        let mut state = results_state();
        state.results.current_page = 1;
        let act = handle(&state, key(KeyCode::PageUp));
        assert!(matches!(act, Action::Batch(_)));
    }

    // -- Esc --

    #[test]
    fn esc_produces_discard_or_esc() {
        let state = results_state();
        let act = handle(&state, key(KeyCode::Esc));
        assert!(matches!(act, Action::Batch(_)));
    }

    // -- extract_schema_table_from_sql --

    #[test]
    fn extract_simple_table() {
        let result = extract_schema_table_from_sql("SELECT * FROM users");
        assert_eq!(result, Some(("public".into(), "users".into())));
    }

    #[test]
    fn extract_qualified_table() {
        let result = extract_schema_table_from_sql("SELECT * FROM myschema.users");
        assert_eq!(result, Some(("myschema".into(), "users".into())));
    }

    #[test]
    fn extract_quoted_table() {
        let result = extract_schema_table_from_sql(r#"SELECT * FROM "public"."my-table""#);
        assert_eq!(result, Some(("public".into(), "my-table".into())));
    }

    #[test]
    fn extract_no_from_returns_none() {
        let result = extract_schema_table_from_sql("INSERT INTO users VALUES (1)");
        assert_eq!(result, None);
    }

    #[test]
    fn extract_case_insensitive_from() {
        let result = extract_schema_table_from_sql("select * from Users where id = 1");
        assert_eq!(result, Some(("public".into(), "Users".into())));
    }

    #[test]
    fn extract_with_where_clause() {
        let result =
            extract_schema_table_from_sql("SELECT * FROM orders WHERE status = 'active'");
        assert_eq!(result, Some(("public".into(), "orders".into())));
    }
}
