//! Keys for the theme picker.

use crossterm::event::{KeyCode, KeyEvent};

use crate::action::{Action, ThemeAction};
use crate::app::AppState;

/// Keys for the theme picker.
///
/// Movement previews, Enter keeps, Esc puts back what was there. `q` is left
/// out on purpose: it quits everywhere else, and a picker that exits the app
/// because the user reached for a familiar key would be a poor trade.
pub fn handle(_state: &AppState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Action::Theme(ThemeAction::Next),
        KeyCode::Char('k') | KeyCode::Up => Action::Theme(ThemeAction::Prev),
        KeyCode::Enter => Action::Theme(ThemeAction::Confirm),
        KeyCode::Esc => Action::Theme(ThemeAction::Cancel),
        _ => Action::Noop,
    }
}
