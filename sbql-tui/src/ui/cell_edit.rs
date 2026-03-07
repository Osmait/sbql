//! In-place cell edit overlay.
//!
//! A floating `tui-textarea` popup rendered over the selected cell.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear},
    Frame,
};

use crate::app::{CellEditState, LayoutCache, ResultsState};
use crate::ui::theme;

pub fn draw(
    frame: &mut Frame,
    edit: &mut CellEditState,
    layout: &LayoutCache,
    results: &ResultsState,
    screen: Rect,
) {
    let popup = compute_popup_rect(layout, results, screen);

    frame.render_widget(Clear, popup);

    edit.textarea.set_block(
        Block::default()
            .title(format!(
                " Edit: {} (original: \"{}\") — Enter/^S: stage  ^W: commit  Esc: cancel ",
                edit.col_name, edit.original
            ))
            .borders(Borders::ALL)
            .border_style(
                Style::default()
                    .fg(theme::YELLOW)
                    .add_modifier(Modifier::BOLD),
            ),
    );
    edit.textarea
        .set_cursor_style(Style::default().bg(theme::YELLOW).fg(theme::BASE));
    edit.textarea
        .set_cursor_line_style(Style::default().bg(theme::SURFACE0));

    frame.render_widget(&edit.textarea, popup);
}

/// Compute the rect for the cell-edit popup.
fn compute_popup_rect(layout: &LayoutCache, results: &ResultsState, screen: Rect) -> Rect {
    const POPUP_WIDTH: u16 = 50;
    const POPUP_HEIGHT: u16 = 5;
    const COL_SPACING: u16 = 1;

    let fallback = Rect {
        x: screen.width / 4,
        y: screen.height / 3,
        width: POPUP_WIDTH.min(screen.width),
        height: POPUP_HEIGHT,
    };

    let Some(la) = layout.last_areas else {
        return fallback;
    };

    if layout.last_col_widths.is_empty() {
        return fallback;
    }

    let col_scroll = results.col_scroll;
    let selected_col = results.selected_col;
    let selected_row = results.selected_row;
    let row_scroll = results.scroll;

    let x_offset: u16 = layout
        .last_col_widths
        .iter()
        .enumerate()
        .skip(col_scroll)
        .take(selected_col.saturating_sub(col_scroll))
        .map(|(_, &w)| w + COL_SPACING)
        .sum();

    let cell_x = la.results.x.saturating_add(1).saturating_add(x_offset);

    let row_offset = selected_row.saturating_sub(row_scroll) as u16;
    let cell_y = la.results.y.saturating_add(2).saturating_add(row_offset);

    let max_x = screen.width.saturating_sub(POPUP_WIDTH);
    let max_y = screen.height.saturating_sub(POPUP_HEIGHT);
    let x = cell_x.min(max_x);
    let y = cell_y.min(max_y);

    Rect {
        x,
        y,
        width: POPUP_WIDTH,
        height: POPUP_HEIGHT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, LastAreas};

    #[test]
    fn test_compute_popup_rect_fallback() {
        let state = AppState::new(vec![]);
        let screen = Rect::new(0, 0, 100, 50);
        let popup = compute_popup_rect(&state.layout, &state.results, screen);
        assert_eq!(popup.width, 50);
        assert_eq!(popup.height, 5);
        assert_eq!(popup.x, 25);
        assert_eq!(popup.y, 16);
    }

    #[test]
    fn test_compute_popup_rect_with_layout() {
        let mut state = AppState::new(vec![]);
        state.layout.last_areas = Some(LastAreas {
            conn_list: Rect::default(),
            table_list: Rect::default(),
            editor: Rect::default(),
            results: Rect::new(10, 10, 80, 20),
        });
        state.layout.last_col_widths = vec![10, 10, 10];
        state.results.selected_col = 1;
        state.results.selected_row = 2;

        let screen = Rect::new(0, 0, 100, 50);
        let popup = compute_popup_rect(&state.layout, &state.results, screen);

        assert_eq!(popup.width, 50);
        assert_eq!(popup.height, 5);
        assert_eq!(popup.x, 22);
        assert_eq!(popup.y, 14);
    }
}
