//! Shared test helpers for sbql-tui tests.

#![cfg(test)]

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use tokio::sync::mpsc;

use crate::action;
use crate::app::AppState;
use crate::handlers;
use sbql_core::{CoreCommand, QueryResult};

/// Create a simple key press event.
pub fn key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    }
}

/// Create a key press event with modifiers.
pub fn key_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    }
}

/// Dispatch a key through `handle_key` → `apply`.
pub fn dispatch(state: &mut AppState, k: KeyEvent, cmd_tx: &mpsc::UnboundedSender<CoreCommand>) {
    let act = handlers::handle_key(state, k);
    action::apply(act, state, cmd_tx);
}

/// Create an `AppState` with mock results data.
pub fn make_state_with_results() -> AppState {
    let mut state = AppState::new(vec![]);
    state.results.data = QueryResult {
        columns: vec!["id".into(), "name".into(), "email".into()],
        rows: vec![
            vec!["1".into(), "Alice".into(), "alice@test.com".into()],
            vec!["2".into(), "Bob".into(), "bob@test.com".into()],
            vec!["3".into(), "Charlie".into(), "charlie@test.com".into()],
            vec!["4".into(), "Diana".into(), "diana@test.com".into()],
            vec!["5".into(), "Eve".into(), "eve@test.com".into()],
        ],
        page: 0,
        has_next_page: false,
    };
    state.results.viewport_height = 20;
    state.results.viewport_cols = 3;
    state
}

/// Create a command channel pair.
pub fn cmd_channel() -> (
    mpsc::UnboundedSender<CoreCommand>,
    mpsc::UnboundedReceiver<CoreCommand>,
) {
    mpsc::unbounded_channel()
}
