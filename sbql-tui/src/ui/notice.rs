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

pub(crate) fn draw(
    frame: &mut Frame,
    notice: &Notice,
    area: Rect,
    hits: &mut crate::ui::hit::HitMap,
) {
    let popup = centred(area, WIDTH_PCT, HEIGHT_PCT);

    let (title, accent) = match notice.level {
        Level::Error => (" Error ", theme::red()),
        Level::Warning => (" Warning ", theme::yellow()),
        Level::Info => (" Message ", theme::green()),
    };

    let mut lines = vec![Line::from(Span::styled(
        notice.text.clone(),
        Style::default()
            .fg(theme::text())
            .add_modifier(Modifier::BOLD),
    ))];

    if let Some(detail) = &notice.detail {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            detail.clone(),
            Style::default().fg(theme::subtext0()),
        )));
    }

    if let Some(hint) = notice.hint() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("→ {hint}"),
            Style::default().fg(theme::sapphire()),
        )));
    }

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Span::styled(
            " any key to close ",
            Style::default().fg(theme::overlay0()),
        ))
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(theme::base()));

    // Wrapping is the point: this exists because the status bar cannot.
    let body = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(Clear, popup);
    frame.render_widget(body, popup);

    // One zone over the whole popup: the overlay is opaque (it clears what it
    // covers) and the only thing to do with it is dismiss it, so every point in
    // it means the same thing — and none of them may reach the panel beneath.
    hits.register(popup, crate::ui::hit::Zone::NoticeDetail);
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
            .draw(|frame| {
                draw(
                    frame,
                    notice,
                    frame.area(),
                    &mut crate::ui::hit::HitMap::default(),
                );
            })
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

    /// The overlay is registered after the panels it covers, so it answers the
    /// click instead of the results grid showing through it. The corners of the
    /// registered rect are checked against the painted border, because a zone
    /// that does not line up with the pixels is the bug this guards.
    #[test]
    fn the_overlay_takes_the_click_from_whatever_is_under_it() {
        use crate::app::FocusedPanel;
        use crate::ui::hit::{HitMap, Zone};

        let notice = Notice::error("boom", 0);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("backend");

        let mut hits = HitMap::default();
        // Stands in for the panel bodies the root draw registers first.
        hits.register(Rect::new(0, 0, 80, 24), Zone::Panel(FocusedPanel::Results));
        terminal
            .draw(|frame| draw(frame, &notice, frame.area(), &mut hits))
            .expect("draw");

        let buffer = terminal.backend().buffer();
        let (rect, zone) = hits.hit(40, 12).expect("a hit in the middle of the popup");
        assert_eq!(zone, Zone::NoticeDetail);
        assert_eq!(
            buffer.cell((rect.x, rect.y)).map(|c| c.symbol()),
            Some("┌"),
            "the zone does not start where the popup was painted"
        );
        assert_eq!(
            buffer
                .cell((rect.right() - 1, rect.bottom() - 1))
                .map(|c| c.symbol()),
            Some("┘"),
            "the zone does not end where the popup was painted"
        );

        // Outside it, the panel underneath still answers.
        assert_eq!(
            hits.hit(0, 0).map(|(_, z)| z),
            Some(Zone::Panel(FocusedPanel::Results))
        );
    }

    /// Overlays get drawn at whatever size the terminal happens to be.
    #[test]
    fn a_tiny_terminal_does_not_panic() {
        let notice = Notice::error("boom", 0);
        for (w, h) in [(1, 1), (2, 3), (10, 4)] {
            drop(render(&notice, w, h));
        }
    }
}
