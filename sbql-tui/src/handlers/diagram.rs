use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;
use crate::app::AppState;

pub fn handle(state: &AppState, key: KeyEvent) -> Action {
    // If search is active, intercept all keys for the search input
    if let Some(ref diag) = state.diagram {
        if diag.search_active {
            return match key.code {
                KeyCode::Esc => Action::DiagramSearchClose,
                KeyCode::Enter => Action::DiagramSearchConfirm,
                KeyCode::Backspace => Action::DiagramSearchBackspace,
                KeyCode::Char(c) => Action::DiagramSearchInput(c),
                _ => Action::Noop,
            };
        }
    }

    // Ctrl modifiers for fast scrolling
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Action::CloseDiagram,

        // Fast scroll with Ctrl
        KeyCode::Char('h') if ctrl => Action::DiagramScroll { dx: -20, dy: 0 },
        KeyCode::Char('l') if ctrl => Action::DiagramScroll { dx: 20, dy: 0 },
        KeyCode::Char('j') if ctrl => Action::DiagramScroll { dx: 0, dy: 10 },
        KeyCode::Char('k') if ctrl => Action::DiagramScroll { dx: 0, dy: -10 },

        // Normal scroll
        KeyCode::Left | KeyCode::Char('h') => Action::DiagramScroll { dx: -4, dy: 0 },
        KeyCode::Right | KeyCode::Char('l') => Action::DiagramScroll { dx: 4, dy: 0 },
        KeyCode::Up => Action::DiagramScroll { dx: 0, dy: -1 },
        KeyCode::Down => Action::DiagramScroll { dx: 0, dy: 1 },

        // Page Up/Down for fast vertical scroll
        KeyCode::PageUp => Action::DiagramScroll { dx: 0, dy: -20 },
        KeyCode::PageDown => Action::DiagramScroll { dx: 0, dy: 20 },

        // Selection
        KeyCode::Char('j') | KeyCode::Tab => Action::DiagramSelectNext,
        KeyCode::Char('k') | KeyCode::BackTab => Action::DiagramSelectPrev,
        KeyCode::Char('g') => Action::DiagramSelectFirst,
        KeyCode::Char('G') => Action::DiagramSelectLast,

        // Toggles
        KeyCode::Char('f') => Action::DiagramToggleFocus,
        KeyCode::Char('u') => Action::DiagramToggleGlyph,

        // Search
        KeyCode::Char('/') => Action::DiagramSearchOpen,

        // Jump
        KeyCode::Enter | KeyCode::Char(' ') => Action::DiagramJumpToTable,
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
        assert!(matches!(act, Action::CloseDiagram));
    }

    #[test]
    fn q_closes() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('q')));
        assert!(matches!(act, Action::CloseDiagram));
    }

    #[test]
    fn h_scrolls_left() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('h')));
        assert!(matches!(act, Action::DiagramScroll { dx: -4, dy: 0 }));
    }

    #[test]
    fn l_scrolls_right() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('l')));
        assert!(matches!(act, Action::DiagramScroll { dx: 4, dy: 0 }));
    }

    #[test]
    fn up_scrolls_up() {
        let s = state();
        let act = handle(&s, key(KeyCode::Up));
        assert!(matches!(act, Action::DiagramScroll { dx: 0, dy: -1 }));
    }

    #[test]
    fn down_scrolls_down() {
        let s = state();
        let act = handle(&s, key(KeyCode::Down));
        assert!(matches!(act, Action::DiagramScroll { dx: 0, dy: 1 }));
    }

    #[test]
    fn j_selects_next() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('j')));
        assert!(matches!(act, Action::DiagramSelectNext));
    }

    #[test]
    fn k_selects_prev() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('k')));
        assert!(matches!(act, Action::DiagramSelectPrev));
    }

    #[test]
    fn g_selects_first() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('g')));
        assert!(matches!(act, Action::DiagramSelectFirst));
    }

    #[test]
    fn shift_g_selects_last() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('G')));
        assert!(matches!(act, Action::DiagramSelectLast));
    }

    #[test]
    fn f_toggles_focus() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('f')));
        assert!(matches!(act, Action::DiagramToggleFocus));
    }

    #[test]
    fn u_toggles_glyph() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('u')));
        assert!(matches!(act, Action::DiagramToggleGlyph));
    }

    #[test]
    fn enter_jumps_to_table() {
        let s = state();
        let act = handle(&s, key(KeyCode::Enter));
        assert!(matches!(act, Action::DiagramJumpToTable));
    }

    #[test]
    fn space_jumps_to_table() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char(' ')));
        assert!(matches!(act, Action::DiagramJumpToTable));
    }

    #[test]
    fn slash_opens_search() {
        let s = state();
        let act = handle(&s, key(KeyCode::Char('/')));
        assert!(matches!(act, Action::DiagramSearchOpen));
    }

    #[test]
    fn page_down_fast_scroll() {
        let s = state();
        let act = handle(&s, key(KeyCode::PageDown));
        assert!(matches!(act, Action::DiagramScroll { dx: 0, dy: 20 }));
    }

    #[test]
    fn page_up_fast_scroll() {
        let s = state();
        let act = handle(&s, key(KeyCode::PageUp));
        assert!(matches!(act, Action::DiagramScroll { dx: 0, dy: -20 }));
    }

    #[test]
    fn ctrl_h_fast_scroll_left() {
        let s = state();
        let k = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL);
        let act = handle(&s, k);
        assert!(matches!(act, Action::DiagramScroll { dx: -20, dy: 0 }));
    }

    #[test]
    fn ctrl_l_fast_scroll_right() {
        let s = state();
        let k = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL);
        let act = handle(&s, k);
        assert!(matches!(act, Action::DiagramScroll { dx: 20, dy: 0 }));
    }

    #[test]
    fn search_mode_intercepts_chars() {
        let mut s = state_with_diagram();
        s.diagram.as_mut().unwrap().search_active = true;
        let act = handle(&s, key(KeyCode::Char('a')));
        assert!(matches!(act, Action::DiagramSearchInput('a')));
    }

    #[test]
    fn search_mode_esc_closes() {
        let mut s = state_with_diagram();
        s.diagram.as_mut().unwrap().search_active = true;
        let act = handle(&s, key(KeyCode::Esc));
        assert!(matches!(act, Action::DiagramSearchClose));
    }

    #[test]
    fn search_mode_enter_confirms() {
        let mut s = state_with_diagram();
        s.diagram.as_mut().unwrap().search_active = true;
        let act = handle(&s, key(KeyCode::Enter));
        assert!(matches!(act, Action::DiagramSearchConfirm));
    }

    #[test]
    fn search_mode_backspace() {
        let mut s = state_with_diagram();
        s.diagram.as_mut().unwrap().search_active = true;
        let act = handle(&s, key(KeyCode::Backspace));
        assert!(matches!(act, Action::DiagramSearchBackspace));
    }
}
