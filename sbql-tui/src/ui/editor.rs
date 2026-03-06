//! Center-top panel — SQL editor (tui-textarea).

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
    Frame,
};

use crate::app::{AppState, EditorMode, FocusedPanel};
use crate::ui::theme;

pub fn draw(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let is_focused = state.focused == FocusedPanel::Editor;

    let border_style = if is_focused {
        Style::default().fg(theme::YELLOW)
    } else {
        Style::default().fg(theme::OVERLAY0)
    };

    // Build the title with connection indicator
    let conn_indicator = match state.active_connection_id {
        Some(id) => {
            let name = state
                .connections
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.name.as_str())
                .unwrap_or("connected");
            format!(" SQL Editor [{}] ", name)
        }
        None => " SQL Editor (no connection) ".into(),
    };

    // Mode badge — only shown when focused
    let mode_span = if is_focused {
        match state.editor_mode {
            EditorMode::Normal => Span::styled(
                " NORMAL ",
                Style::default()
                    .fg(theme::BASE)
                    .bg(theme::MAUVE)
                    .add_modifier(Modifier::BOLD),
            ),
            EditorMode::Insert => Span::styled(
                " INSERT ",
                Style::default()
                    .fg(theme::BASE)
                    .bg(theme::GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
        }
    } else {
        Span::raw("")
    };

    let help_span = if is_focused {
        match state.editor_mode {
            EditorMode::Normal => Span::styled(
                " hjkl: move  i: insert  M-hjkl: panels  ^S/F5: run ",
                Style::default().fg(theme::OVERLAY0),
            ),
            EditorMode::Insert => Span::styled(
                " Esc: normal  M-hjkl: panels  ^S/F5: run ",
                Style::default().fg(theme::OVERLAY0),
            ),
        }
    } else {
        Span::styled(" Tab/click: focus ", Style::default().fg(theme::OVERLAY0))
    };

    let title = Line::from(vec![
        Span::styled(
            conn_indicator,
            Style::default()
                .fg(theme::YELLOW)
                .add_modifier(Modifier::BOLD),
        ),
        mode_span,
        help_span,
    ]);

    state.editor.set_block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style),
    );

    // Style the cursor line
    if is_focused {
        state
            .editor
            .set_cursor_line_style(Style::default().bg(theme::SURFACE0));
        // Cursor color differs by mode: yellow (Normal) vs green (Insert)
        let cursor_style = match state.editor_mode {
            EditorMode::Normal => Style::default().bg(theme::MAUVE).fg(theme::BASE),
            EditorMode::Insert => Style::default().bg(theme::YELLOW).fg(theme::BASE),
        };
        state.editor.set_cursor_style(cursor_style);
    } else {
        state.editor.set_cursor_line_style(Style::default());
        state.editor.set_cursor_style(Style::default());
    }

    frame.render_widget(&state.editor, area);
}
