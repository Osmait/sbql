//! Center-top panel — SQL editor (tui-textarea).

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
    Frame,
};

use crate::app::{ConnectionState, EditorMode, EditorState, FocusedPanel};
use crate::ui::theme;

pub fn draw(
    frame: &mut Frame,
    editor: &mut EditorState,
    conn: &ConnectionState,
    focused: FocusedPanel,
    area: Rect,
) {
    let is_focused = focused == FocusedPanel::Editor;

    let border_style = if is_focused {
        Style::default().fg(theme::YELLOW)
    } else {
        Style::default().fg(theme::OVERLAY0)
    };

    let conn_indicator = match conn.active_id {
        Some(id) => {
            let name = conn
                .connections
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.name.as_str())
                .unwrap_or("connected");
            format!(" SQL Editor [{}] ", name)
        }
        None => " SQL Editor (no connection) ".into(),
    };

    let mode_span = if is_focused {
        match editor.mode {
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
        match editor.mode {
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

    editor.textarea.set_block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style),
    );

    if is_focused {
        editor
            .textarea
            .set_cursor_line_style(Style::default().bg(theme::SURFACE0));
        let cursor_style = match editor.mode {
            EditorMode::Normal => Style::default().bg(theme::MAUVE).fg(theme::BASE),
            EditorMode::Insert => Style::default().bg(theme::YELLOW).fg(theme::BASE),
        };
        editor.textarea.set_cursor_style(cursor_style);
    } else {
        editor
            .textarea
            .set_cursor_line_style(Style::default());
        editor.textarea.set_cursor_style(Style::default());
    }

    frame.render_widget(&editor.textarea, area);
}
