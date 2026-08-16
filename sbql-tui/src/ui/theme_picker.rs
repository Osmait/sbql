//! The theme picker overlay.
//!
//! Deliberately plain: the theme is already applied as the cursor moves, so
//! the preview is the entire application behind this box. A panel of swatches
//! would be a worse likeness of a theme than the thing itself.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

use crate::app::ThemePicker;
use crate::ui::hit::{HitMap, Zone};
use crate::ui::theme;

/// Draw the picker over `screen`.
pub(crate) fn draw(frame: &mut Frame, picker: &ThemePicker, screen: Rect, hits: &mut HitMap) {
    let height = u16::try_from(theme::THEMES.len()).unwrap_or(u16::MAX) + 2;
    let area = centered(34, height.min(screen.height), screen);

    frame.render_widget(Clear, area);

    let items: Vec<ListItem> = theme::THEMES
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let selected = i == picker.selected();
            let marker = if selected { "▸ " } else { "  " };
            let style = if selected {
                Style::default()
                    .fg(theme::base())
                    .bg(theme::mauve())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme::text())
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(theme::mauve())),
                Span::styled(t.name, style),
            ]))
        })
        .collect();

    let inner = area.inner(ratatui::layout::Margin::new(1, 1));
    for i in 0..theme::THEMES.len() {
        let Ok(offset) = u16::try_from(i) else { break };
        let y = inner.y + offset;
        if y >= inner.bottom() {
            break;
        }
        hits.register(Rect::new(inner.x, y, inner.width, 1), Zone::ThemeRow(i));
    }

    let list = List::new(items).block(
        Block::default()
            .title(" Theme  (j/k preview · Enter keep · Esc cancel) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::mauve())),
    );

    let mut state = ListState::default();
    state.select(Some(picker.selected()));
    frame.render_stateful_widget(list, area, &mut state);
}

/// A box of `width` × `height` in the middle of `screen`.
fn centered(width: u16, height: u16, screen: Rect) -> Rect {
    let w = width.min(screen.width);
    let h = height.min(screen.height);
    Rect {
        x: screen.x + (screen.width - w) / 2,
        y: screen.y + (screen.height - h) / 2,
        width: w,
        height: h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    /// Every theme has to be reachable, and the one under the cursor marked.
    #[test]
    fn the_picker_lists_every_theme_and_marks_the_current_one() {
        let mut terminal = Terminal::new(TestBackend::new(80, 30)).unwrap();
        let picker = ThemePicker::open();
        let mut hits = HitMap::default();

        terminal
            .draw(|f| draw(f, &picker, f.area(), &mut hits))
            .unwrap();
        let content: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();

        for t in theme::THEMES {
            assert!(content.contains(t.name), "{} is not listed", t.name);
        }
        assert!(content.contains('▸'), "the cursor is not marked");
    }

    /// Clicking a row has to land on the theme painted there.
    #[test]
    fn every_listed_row_is_clickable() {
        let mut terminal = Terminal::new(TestBackend::new(80, 30)).unwrap();
        let picker = ThemePicker::open();
        let mut hits = HitMap::default();
        terminal
            .draw(|f| draw(f, &picker, f.area(), &mut hits))
            .unwrap();

        let rows: Vec<usize> = (0..80)
            .flat_map(|x| (0..30).map(move |y| (x, y)))
            .filter_map(|(x, y)| match hits.hit(x, y) {
                Some((_, Zone::ThemeRow(i))) => Some(i),
                _ => None,
            })
            .collect();

        for i in 0..theme::THEMES.len() {
            assert!(rows.contains(&i), "theme row {i} takes no clicks");
        }
    }

    /// A terminal too short for the list must still draw something valid.
    #[test]
    fn a_tiny_terminal_does_not_panic() {
        for (w, h) in [(20, 4), (10, 3), (40, 2)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            let picker = ThemePicker::open();
            let mut hits = HitMap::default();
            terminal
                .draw(|f| draw(f, &picker, f.area(), &mut hits))
                .unwrap();
        }
    }
}
