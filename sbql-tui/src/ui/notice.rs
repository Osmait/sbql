//! The full text of a status-bar message.
//!
//! The bar is one row and does not wrap. That is fine for "Connected to prod"
//! and useless for `ERROR: syntax error at or near ")" LINE 1: ...`, which is
//! the message a SQL client most needs to show. This overlay is where the whole
//! thing lives: the summary, the cause chain the core kept for us, and what to
//! do about it.

use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use super::theme;
use crate::notice::{Level, Notice};

/// How much of the screen the overlay may take, as a percentage.
const WIDTH_PCT: u16 = 70;
const HEIGHT_PCT: u16 = 50;

pub fn draw(frame: &mut Frame, notice: &Notice, area: Rect) {
    let popup = centred(area, WIDTH_PCT, HEIGHT_PCT);

    let (title, accent) = match notice.level {
        Level::Error => (" Error ", theme::RED),
        Level::Warning => (" Warning ", theme::YELLOW),
        Level::Info => (" Message ", theme::GREEN),
    };

    let mut lines = vec![Line::from(Span::styled(
        notice.text.clone(),
        Style::default()
            .fg(theme::TEXT)
            .add_modifier(Modifier::BOLD),
    ))];

    if let Some(detail) = &notice.detail {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            detail.clone(),
            Style::default().fg(theme::SUBTEXT0),
        )));
    }

    if let Some(hint) = notice.hint() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("→ {hint}"),
            Style::default().fg(theme::SAPPHIRE),
        )));
    }

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Span::styled(
            " any key to close ",
            Style::default().fg(theme::OVERLAY0),
        ))
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(theme::BASE));

    // Wrapping is the point: this exists because the status bar cannot.
    let body = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(Clear, popup);
    frame.render_widget(body, popup);
}

/// A rect of `pct_x` × `pct_y` percent, centred in `area`.
fn centred(area: Rect, pct_x: u16, pct_y: u16) -> Rect {
    let width = (area.width * pct_x / 100).clamp(1, area.width);
    let height = (area.height * pct_y / 100).clamp(1, area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};
    use sbql_core::{CoreError, ErrorKind};

    fn render(notice: &Notice, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("backend");
        terminal
            .draw(|frame| draw(frame, notice, frame.area()))
            .expect("draw");
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    /// The reason the overlay exists: a message the one-row bar had to cut off
    /// is readable here in full.
    #[test]
    fn a_long_message_is_shown_whole() {
        let long = "syntax error at or near \")\" — the statement could not be parsed \
                    anywhere near the end of it";
        let notice = Notice::from_core(CoreError::new(ErrorKind::Query, long), 0);

        let screen = render(&notice, 100, 20);
        assert!(screen.contains("syntax error at or near"), "{screen}");
        assert!(screen.contains("could not be parsed"), "the tail was cut");
    }

    #[test]
    fn the_cause_and_the_next_step_are_both_shown() {
        let notice = Notice::from_core(
            CoreError::new(ErrorKind::NoActiveConnection, "No active connection"),
            0,
        );

        let screen = render(&notice, 100, 20);
        assert!(screen.contains("No active connection"), "{screen}");
        assert!(screen.contains("press Enter"), "no hint shown:\n{screen}");
    }

    #[test]
    fn a_warning_is_not_titled_error() {
        let notice = Notice::from_core(
            CoreError::warning(ErrorKind::Credentials, "saved, password not stored"),
            0,
        );

        let screen = render(&notice, 100, 20);
        assert!(screen.contains("Warning"), "{screen}");
        assert!(!screen.contains("Error"), "{screen}");
    }

    /// Overlays get drawn at whatever size the terminal happens to be.
    #[test]
    fn a_tiny_terminal_does_not_panic() {
        let notice = Notice::error("boom", 0);
        for (w, h) in [(1, 1), (2, 3), (10, 4)] {
            let _ = render(&notice, w, h);
        }
    }
}
