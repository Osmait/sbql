//! The theme picker.

use super::*;
use crate::app::ThemePicker;
use crate::ui::theme as palette;

/// The theme picker.
pub(super) fn apply(action: ThemeAction, state: &mut AppState) {
    match action {
        ThemeAction::Open => {
            state.close_overlays();
            state.conn.form.visible = false;
            state.theme_picker = ThemePicker::open();
            state.inform("Theme: j/k to preview, Enter to keep, Esc to cancel");
        }

        // Moving applies straight away: a swatch cannot tell you what a theme
        // reads like, and the app itself can.
        ThemeAction::Next => {
            state
                .theme_picker
                .cursor
                .next(palette::THEMES.len(), Overflow::Wrap);
            palette::set(state.theme_picker.selected());
        }

        ThemeAction::Prev => {
            state
                .theme_picker
                .cursor
                .prev(palette::THEMES.len(), Overflow::Wrap);
            palette::set(state.theme_picker.selected());
        }

        ThemeAction::Select(idx) => {
            state.theme_picker.cursor.select(idx, palette::THEMES.len());
            palette::set(state.theme_picker.selected());
        }

        ThemeAction::Confirm => {
            let name = palette::current_name();
            state.theme_picker.visible = false;
            // Best-effort, like the rest of the session file: failing to
            // remember a colour scheme is not worth interrupting anyone.
            crate::session::remember_theme(name);
            state.inform(format!("Theme: {name}"));
        }

        ThemeAction::Cancel => {
            palette::set(state.theme_picker.previous);
            state.theme_picker.visible = false;
            state.inform("Theme unchanged");
        }
    }
}
