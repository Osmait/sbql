use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::{Action, DiagramAction};
use crate::app::AppState;

pub(crate) fn handle(state: &AppState, key: KeyEvent) -> Action {
    // If search is active, intercept all keys for the search input
    if let Some(ref diag) = state.diagram {
        if diag.search_active {
            return match key.code {
                KeyCode::Esc => Action::Diagram(DiagramAction::SearchClose),
                KeyCode::Enter => Action::Diagram(DiagramAction::SearchConfirm),
                KeyCode::Backspace => Action::Diagram(DiagramAction::SearchBackspace),
                KeyCode::Char(c) => Action::Diagram(DiagramAction::SearchInput(c)),
                _ => Action::Noop,
            };
        }
    }

    // Ctrl modifiers for fast scrolling
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Action::Diagram(DiagramAction::Close),

        // Fast scroll with Ctrl
        KeyCode::Char('h') if ctrl => Action::Diagram(DiagramAction::Scroll { dx: -20, dy: 0 }),
        KeyCode::Char('l') if ctrl => Action::Diagram(DiagramAction::Scroll { dx: 20, dy: 0 }),
        KeyCode::Char('j') if ctrl => Action::Diagram(DiagramAction::Scroll { dx: 0, dy: 10 }),
        KeyCode::Char('k') if ctrl => Action::Diagram(DiagramAction::Scroll { dx: 0, dy: -10 }),

        // Normal scroll
        KeyCode::Left | KeyCode::Char('h') => {
            Action::Diagram(DiagramAction::Scroll { dx: -4, dy: 0 })
        }
        KeyCode::Right | KeyCode::Char('l') => {
            Action::Diagram(DiagramAction::Scroll { dx: 4, dy: 0 })
        }
        KeyCode::Up => Action::Diagram(DiagramAction::Scroll { dx: 0, dy: -1 }),
        KeyCode::Down => Action::Diagram(DiagramAction::Scroll { dx: 0, dy: 1 }),

        // Page Up/Down for fast vertical scroll
        KeyCode::PageUp => Action::Diagram(DiagramAction::Scroll { dx: 0, dy: -20 }),
        KeyCode::PageDown => Action::Diagram(DiagramAction::Scroll { dx: 0, dy: 20 }),

        // Selection
        KeyCode::Char('j') | KeyCode::Tab => Action::Diagram(DiagramAction::SelectNext),
        KeyCode::Char('k') | KeyCode::BackTab => Action::Diagram(DiagramAction::SelectPrev),
        KeyCode::Char('g') => Action::Diagram(DiagramAction::SelectFirst),
        KeyCode::Char('G') => Action::Diagram(DiagramAction::SelectLast),

        // Toggles
        KeyCode::Char('f') => Action::Diagram(DiagramAction::ToggleFocus),
        KeyCode::Char('u') => Action::Diagram(DiagramAction::ToggleGlyph),

        // Search
        KeyCode::Char('/') => Action::Diagram(DiagramAction::SearchOpen),

        // Jump
        KeyCode::Enter | KeyCode::Char(' ') => Action::Diagram(DiagramAction::JumpToTable),
        _ => Action::Noop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::key;
    use sbql_core::DiagramData;

    fn state() -> AppState {
        AppState::new(vec![])
    }

    fn state_with_diagram() -> AppState {
        let mut s = AppState::new(vec![]);
        s.diagram = Some(crate::app::DiagramState::new(DiagramData {
            tables: vec![],
            foreign_keys: vec![],
        }));
        s
    }

    #[test]
    fn esc_closes() {
        let s = state();
        let act = handle(&s, key(KeyCode::Esc));
        assert!(matches!(act, Action::Diagram(DiagramAction::Close)));
    }

    #[test]
    fn q_closes() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('q')));
        assert!(matches!(act, Action::Diagram(DiagramAction::Close)));
    }

    #[test]
    fn h_scrolls_left() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('h')));
        assert!(matches!(
            act,
            Action::Diagram(DiagramAction::Scroll { dx: -4, dy: 0 })
        ));
    }

    #[test]
    fn l_scrolls_right() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('l')));
        assert!(matches!(
            act,
            Action::Diagram(DiagramAction::Scroll { dx: 4, dy: 0 })
        ));
    }

    #[test]
    fn up_scrolls_up() {
        let s = state();
        let act = handle(&s, key(KeyCode::Up));
        assert!(matches!(
            act,
            Action::Diagram(DiagramAction::Scroll { dx: 0, dy: -1 })
        ));
    }

    #[test]
    fn down_scrolls_down() {
        let s = state();
        let act = handle(&s, key(KeyCode::Down));
        assert!(matches!(
            act,
            Action::Diagram(DiagramAction::Scroll { dx: 0, dy: 1 })
        ));
    }

    #[test]
    fn j_selects_next() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('j')));
        assert!(matches!(act, Action::Diagram(DiagramAction::SelectNext)));
    }

    #[test]
    fn k_selects_prev() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('k')));
        assert!(matches!(act, Action::Diagram(DiagramAction::SelectPrev)));
    }

    #[test]
    fn g_selects_first() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('g')));
        assert!(matches!(act, Action::Diagram(DiagramAction::SelectFirst)));
    }

    #[test]
    fn shift_g_selects_last() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('G')));
        assert!(matches!(act, Action::Diagram(DiagramAction::SelectLast)));
    }

    #[test]
    fn f_toggles_focus() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('f')));
        assert!(matches!(act, Action::Diagram(DiagramAction::ToggleFocus)));
    }

    #[test]
    fn u_toggles_glyph() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('u')));
        assert!(matches!(act, Action::Diagram(DiagramAction::ToggleGlyph)));
    }

    #[test]
    fn enter_jumps_to_table() {
        let s = state();
        let act = handle(&s, key(KeyCode::Enter));
        assert!(matches!(act, Action::Diagram(DiagramAction::JumpToTable)));
    }

    #[test]
    fn space_jumps_to_table() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char(' ')));
        assert!(matches!(act, Action::Diagram(DiagramAction::JumpToTable)));
    }

    #[test]
    fn slash_opens_search() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('/')));
        assert!(matches!(act, Action::Diagram(DiagramAction::SearchOpen)));
    }

    #[test]
    fn page_down_fast_scroll() {
        let s = state();
        let act = handle(&s, key(KeyCode::PageDown));
        assert!(matches!(
            act,
            Action::Diagram(DiagramAction::Scroll { dx: 0, dy: 20 })
        ));
    }

    #[test]
    fn page_up_fast_scroll() {
        let s = state();
        let act = handle(&s, key(KeyCode::PageUp));
        assert!(matches!(
            act,
            Action::Diagram(DiagramAction::Scroll { dx: 0, dy: -20 })
        ));
    }

    #[test]
    fn ctrl_h_fast_scroll_left() {
        let s = state();
        let k = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL);
        let act = handle(&s, k);
        assert!(matches!(
            act,
            Action::Diagram(DiagramAction::Scroll { dx: -20, dy: 0 })
        ));
    }

    #[test]
    fn ctrl_l_fast_scroll_right() {
        let s = state();
        let k = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL);
        let act = handle(&s, k);
        assert!(matches!(
            act,
            Action::Diagram(DiagramAction::Scroll { dx: 20, dy: 0 })
        ));
    }

    #[test]
    fn search_mode_intercepts_chars() {
        let mut s = state_with_diagram();
        s.diagram.as_mut().unwrap().search_active = true;
        let act = handle(&s, key(KeyCode::Char('a')));
        assert!(matches!(
            act,
            Action::Diagram(DiagramAction::SearchInput('a'))
        ));
    }

    #[test]
    fn search_mode_esc_closes() {
        let mut s = state_with_diagram();
        s.diagram.as_mut().unwrap().search_active = true;
        let act = handle(&s, key(KeyCode::Esc));
        assert!(matches!(act, Action::Diagram(DiagramAction::SearchClose)));
    }

    #[test]
    fn search_mode_enter_confirms() {
        let mut s = state_with_diagram();
        s.diagram.as_mut().unwrap().search_active = true;
        let act = handle(&s, key(KeyCode::Enter));
        assert!(matches!(act, Action::Diagram(DiagramAction::SearchConfirm)));
    }

    #[test]
    fn search_mode_backspace() {
        let mut s = state_with_diagram();
        s.diagram.as_mut().unwrap().search_active = true;
        let act = handle(&s, key(KeyCode::Backspace));
        assert!(matches!(
            act,
            Action::Diagram(DiagramAction::SearchBackspace)
        ));
    }
}
