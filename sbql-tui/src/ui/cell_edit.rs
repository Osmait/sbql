//! In-place cell edit overlay.
//!
//! A floating `tui-textarea` popup rendered over the selected cell.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear},
    Frame,
};

use crate::app::AppState;
use crate::ui::theme;

pub fn draw(frame: &mut Frame, state: &mut AppState) {
    if state.cell_edit.is_none() {
        return;
    }

    let area = frame.area();
    let popup = compute_popup_rect(state, area);

    frame.render_widget(Clear, popup);

    let ce = state.cell_edit.as_mut().unwrap();

    ce.textarea.set_block(
        Block::default()
            .title(format!(
                " Edit: {} (original: \"{}\") — Enter/^S: stage  ^W: commit  Esc: cancel ",
                ce.col_name, ce.original
            ))
            .borders(Borders::ALL)
            .border_style(
                Style::default()
                    .fg(theme::YELLOW)
                    .add_modifier(Modifier::BOLD),
            ),
    );
    ce.textarea
        .set_cursor_style(Style::default().bg(theme::YELLOW).fg(theme::BASE));
    ce.textarea
        .set_cursor_line_style(Style::default().bg(theme::SURFACE0));

    frame.render_widget(&ce.textarea, popup);
}

/// Compute the rect for the cell-edit popup.
///
/// Tries to position the popup over the selected cell in the results table.
/// Falls back to a centred position when geometry is unavailable.
fn compute_popup_rect(state: &AppState, screen: Rect) -> Rect {
    const POPUP_WIDTH: u16 = 50;
    const POPUP_HEIGHT: u16 = 5;
    const COL_SPACING: u16 = 1;

    let fallback = Rect {
        x: screen.width / 4,
        y: screen.height / 3,
        width: POPUP_WIDTH.min(screen.width),
        height: POPUP_HEIGHT,
    };

    let Some(la) = state.last_areas else {
        return fallback;
    };

    if state.last_col_widths.is_empty() {
        return fallback;
    }

    let col_scroll = state.result_col_scroll;
    let selected_col = state.selected_col;
    let selected_row = state.selected_row;
    let row_scroll = state.result_scroll;

    // x: results left border + 1 (border) + sum of visible column widths before selected col
    let x_offset: u16 = state
        .last_col_widths
        .iter()
        .enumerate()
        .skip(col_scroll)
        .take(selected_col.saturating_sub(col_scroll))
        .map(|(_, &w)| w + COL_SPACING)
        .sum();

    // +1 for left border
    let cell_x = la.results.x.saturating_add(1).saturating_add(x_offset);

    // y: results top + 1 (top border) + 1 (header row) + (selected_row - row_scroll)
    let row_offset = selected_row.saturating_sub(row_scroll) as u16;
    let cell_y = la.results.y.saturating_add(2).saturating_add(row_offset);

    // Clamp so the popup stays on screen
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
