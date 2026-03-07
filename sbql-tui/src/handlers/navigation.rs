use crossterm::event::{KeyCode, KeyEvent};

use crate::action::Action;
use crate::app::{AppState, FocusedPanel};

pub fn try_navigate_panels(state: &AppState, key: KeyEvent) -> Option<Action> {
    let sidebar = !state.layout.sidebar_hidden;
    match key.code {
        KeyCode::Char('l') | KeyCode::Right => {
            let target = match state.focused {
                FocusedPanel::Connections => Some(FocusedPanel::Editor),
                FocusedPanel::Tables => Some(FocusedPanel::Results),
                _ => None,
            };
            target.map(Action::FocusPanel)
        }
        KeyCode::Char('h') | KeyCode::Left => {
            if sidebar {
                let target = match state.focused {
                    FocusedPanel::Editor => Some(FocusedPanel::Connections),
                    FocusedPanel::Results => Some(FocusedPanel::Tables),
                    _ => None,
                };
                target.map(Action::FocusPanel)
            } else {
                None
            }
        }
        KeyCode::Char('j') | KeyCode::Down => {
            let target = match state.focused {
                FocusedPanel::Connections => Some(FocusedPanel::Tables),
                FocusedPanel::Editor => Some(FocusedPanel::Results),
                _ => None,
            };
            target.map(Action::FocusPanel)
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let target = match state.focused {
                FocusedPanel::Tables => Some(FocusedPanel::Connections),
                FocusedPanel::Results => Some(FocusedPanel::Editor),
                _ => None,
            };
            target.map(Action::FocusPanel)
        }
        _ => None,
    }
}

pub fn tab_next(current: FocusedPanel, sidebar_hidden: bool) -> FocusedPanel {
    match current {
        FocusedPanel::Connections => {
            if sidebar_hidden {
                FocusedPanel::Editor
            } else {
                FocusedPanel::Tables
            }
        }
        FocusedPanel::Tables => FocusedPanel::Editor,
        FocusedPanel::Editor => FocusedPanel::Results,
        FocusedPanel::Results => {
            if sidebar_hidden {
                FocusedPanel::Editor
            } else {
                FocusedPanel::Connections
            }
        }
    }
}

pub fn tab_prev(current: FocusedPanel, sidebar_hidden: bool) -> FocusedPanel {
    match current {
        FocusedPanel::Connections => FocusedPanel::Results,
        FocusedPanel::Tables => FocusedPanel::Connections,
        FocusedPanel::Editor => {
            if sidebar_hidden {
                FocusedPanel::Results
            } else {
                FocusedPanel::Tables
            }
        }
        FocusedPanel::Results => FocusedPanel::Editor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, NavMode};
    use crate::test_helpers::key;
    use crossterm::event::KeyCode;

    // -- tab_next --

    #[test]
    fn tab_next_full_cycle_sidebar_visible() {
        assert_eq!(tab_next(FocusedPanel::Connections, false), FocusedPanel::Tables);
        assert_eq!(tab_next(FocusedPanel::Tables, false), FocusedPanel::Editor);
        assert_eq!(tab_next(FocusedPanel::Editor, false), FocusedPanel::Results);
        assert_eq!(tab_next(FocusedPanel::Results, false), FocusedPanel::Connections);
    }

    #[test]
    fn tab_next_sidebar_hidden_skips_left_panels() {
        assert_eq!(tab_next(FocusedPanel::Connections, true), FocusedPanel::Editor);
        assert_eq!(tab_next(FocusedPanel::Editor, true), FocusedPanel::Results);
        assert_eq!(tab_next(FocusedPanel::Results, true), FocusedPanel::Editor);
    }

    // -- tab_prev --

    #[test]
    fn tab_prev_full_cycle_sidebar_visible() {
        assert_eq!(tab_prev(FocusedPanel::Connections, false), FocusedPanel::Results);
        assert_eq!(tab_prev(FocusedPanel::Tables, false), FocusedPanel::Connections);
        assert_eq!(tab_prev(FocusedPanel::Editor, false), FocusedPanel::Tables);
        assert_eq!(tab_prev(FocusedPanel::Results, false), FocusedPanel::Editor);
    }

    #[test]
    fn tab_prev_sidebar_hidden() {
        assert_eq!(tab_prev(FocusedPanel::Editor, true), FocusedPanel::Results);
        assert_eq!(tab_prev(FocusedPanel::Results, true), FocusedPanel::Editor);
    }

    // -- try_navigate_panels --

    #[test]
    fn navigate_l_from_connections_goes_editor() {
        let state = AppState::new(vec![]);
        let act = try_navigate_panels(&state, key(KeyCode::Char('l')));
        assert!(act.is_some());
    }

    #[test]
    fn navigate_h_from_editor_goes_connections() {
        let mut state = AppState::new(vec![]);
        state.focused = FocusedPanel::Editor;
        let act = try_navigate_panels(&state, key(KeyCode::Char('h')));
        assert!(act.is_some());
    }

    #[test]
    fn navigate_h_from_editor_sidebar_hidden_returns_none() {
        let mut state = AppState::new(vec![]);
        state.focused = FocusedPanel::Editor;
        state.layout.sidebar_hidden = true;
        let act = try_navigate_panels(&state, key(KeyCode::Char('h')));
        assert!(act.is_none());
    }

    #[test]
    fn navigate_j_from_connections_goes_tables() {
        let state = AppState::new(vec![]);
        let act = try_navigate_panels(&state, key(KeyCode::Char('j')));
        assert!(act.is_some());
    }

    #[test]
    fn navigate_k_from_tables_goes_connections() {
        let mut state = AppState::new(vec![]);
        state.focused = FocusedPanel::Tables;
        let act = try_navigate_panels(&state, key(KeyCode::Char('k')));
        assert!(act.is_some());
    }

    #[test]
    fn navigate_k_from_results_goes_editor() {
        let mut state = AppState::new(vec![]);
        state.focused = FocusedPanel::Results;
        let act = try_navigate_panels(&state, key(KeyCode::Char('k')));
        assert!(act.is_some());
    }

    #[test]
    fn navigate_l_from_results_returns_none() {
        let mut state = AppState::new(vec![]);
        state.focused = FocusedPanel::Results;
        let act = try_navigate_panels(&state, key(KeyCode::Char('l')));
        assert!(act.is_none());
    }

    #[test]
    fn navigate_unknown_key_returns_none() {
        let state = AppState::new(vec![]);
        let act = try_navigate_panels(&state, key(KeyCode::Char('x')));
        assert!(act.is_none());
    }
}
