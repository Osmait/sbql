use crossterm::event::{KeyCode, KeyEvent};
use tui_textarea::Input;

use crate::action::Action;
use crate::app::AppState;

pub fn handle(state: &AppState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            if state.filter.show_suggestions {
                Action::FilterCloseSuggestions
            } else {
                Action::FilterClose
            }
        }
        KeyCode::Up => {
            if state.filter.show_suggestions && !state.filter.suggestions.is_empty() {
                Action::FilterSuggestionUp
            } else {
                Action::FilterInput(Input::from(key))
            }
        }
        KeyCode::Down => {
            if state.filter.show_suggestions && !state.filter.suggestions.is_empty() {
                Action::FilterSuggestionDown
            } else {
                Action::FilterInput(Input::from(key))
            }
        }
        KeyCode::Tab => Action::FilterApplySuggestion,
        KeyCode::Enter => {
            // Try to apply suggestion first; if no suggestion applied, apply the filter
            if state.filter.show_suggestions && !state.filter.suggestions.is_empty() {
                // Check if the suggestion would actually change the input
                let choice = state
                    .filter
                    .suggestions
                    .get(state.filter.selected_suggestion)
                    .cloned()
                    .unwrap_or_default();
                let current = state.filter.textarea.lines().join("");
                let replacement = if let Some(colon) = current.find(':') {
                    let col = current[..colon].trim();
                    format!("{col}:{choice}")
                } else {
                    format!("{choice}:")
                };
                if replacement != current {
                    return Action::FilterApplySuggestion;
                }
            }
            Action::FilterApply
        }
        _ => Action::FilterInput(Input::from(key)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::key;

    fn filter_state(show_suggestions: bool, suggestions: &[&str]) -> AppState {
        let mut state = AppState::new(vec![]);
        state.filter.visible = true;
        state.filter.show_suggestions = show_suggestions;
        state.filter.suggestions = suggestions.iter().map(|s| s.to_string()).collect();
        state.filter.selected_suggestion = 0;
        state
    }

    #[test]
    fn esc_with_suggestions_closes_suggestions() {
        let state = filter_state(true, &["col1", "col2"]);
        let act = handle(&state, key(KeyCode::Esc));
        assert!(matches!(act, Action::FilterCloseSuggestions));
    }

    #[test]
    fn esc_without_suggestions_closes_filter() {
        let state = filter_state(false, &[]);
        let act = handle(&state, key(KeyCode::Esc));
        assert!(matches!(act, Action::FilterClose));
    }

    #[test]
    fn up_with_suggestions_moves_up() {
        let state = filter_state(true, &["a", "b"]);
        let act = handle(&state, key(KeyCode::Up));
        assert!(matches!(act, Action::FilterSuggestionUp));
    }

    #[test]
    fn down_with_suggestions_moves_down() {
        let state = filter_state(true, &["a", "b"]);
        let act = handle(&state, key(KeyCode::Down));
        assert!(matches!(act, Action::FilterSuggestionDown));
    }

    #[test]
    fn up_without_suggestions_is_input() {
        let state = filter_state(false, &[]);
        let act = handle(&state, key(KeyCode::Up));
        assert!(matches!(act, Action::FilterInput(_)));
    }

    #[test]
    fn down_without_suggestions_is_input() {
        let state = filter_state(false, &[]);
        let act = handle(&state, key(KeyCode::Down));
        assert!(matches!(act, Action::FilterInput(_)));
    }

    #[test]
    fn tab_applies_suggestion() {
        let state = filter_state(true, &["a"]);
        let act = handle(&state, key(KeyCode::Tab));
        assert!(matches!(act, Action::FilterApplySuggestion));
    }

    #[test]
    fn enter_no_suggestions_applies_filter() {
        let state = filter_state(false, &[]);
        let act = handle(&state, key(KeyCode::Enter));
        assert!(matches!(act, Action::FilterApply));
    }

    #[test]
    fn char_produces_filter_input() {
        let state = filter_state(false, &[]);
        let act = handle(&state, key(KeyCode::Char('a')));
        assert!(matches!(act, Action::FilterInput(_)));
    }

    #[test]
    fn enter_with_suggestions_applies_suggestion_or_filter() {
        let mut state = filter_state(true, &["col1"]);
        // Input is empty → replacement = "col1:", current = "" → different → apply suggestion
        state.filter.textarea = tui_textarea::TextArea::default();
        let act = handle(&state, key(KeyCode::Enter));
        assert!(matches!(
            act,
            Action::FilterApplySuggestion | Action::FilterApply
        ));
    }
}
