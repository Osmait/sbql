//! In-place cell edit overlay.
//!
//! A floating `tui-textarea` popup rendered over the selected cell.

use ratatui::{
    layout::{Margin, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear},
    Frame,
};

use crate::app::{CellEditState, LayoutCache, ResultsState};
// Shared with the connection form's help line: both overlays turn a word in a
// single painted row into a button, and both have to cope with it being cut off.
use crate::ui::connections::{label_rect, width_of};
use crate::ui::hit::Zone;
use crate::ui::theme;

/// The two words in the title bar that are also buttons. Kept apart from the
/// text around them so the hit rects are measured from the strings that get
/// painted, instead of from column counts that drift when the wording changes.
const STAGE_HINT: &str = "Enter/^S: stage";
const COMMIT_HINT: &str = "  ^W: commit  ";
const CANCEL_HINT: &str = "Esc: cancel";

pub fn draw(
    frame: &mut Frame,
    edit: &mut CellEditState,
    layout: &LayoutCache,
    results: &ResultsState,
    screen: Rect,
    hits: &mut crate::ui::hit::HitMap,
) {
    let popup = compute_popup_rect(layout, results, screen);

    frame.render_widget(Clear, popup);

    // The hints live on the bottom border, not beside the cell's name on the
    // top one. The popup is a fixed 50 columns and the name and old value are
    // arbitrary length, so sharing a line with them meant "commit" and
    // "cancel" were clipped away for every cell — the two keys a user in a
    // half-finished edit most needs to be told about. The bottom border was
    // empty and the hints fit it with room to spare.
    let title = format!(
        " Edit: {} (original: \"{}\") ",
        edit.col_name, edit.original
    );
    edit.textarea.set_block(
        Block::default()
            .title(title)
            .title_bottom(format!(" {STAGE_HINT}{COMMIT_HINT}{CANCEL_HINT} "))
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

    // The text area, so a click inside the open editor stops at it instead of
    // reaching the results grid it is floating over and moving the selection
    // out from under the cell being edited.
    hits.register(popup.inner(Margin::new(1, 1)), Zone::CellEditInput);

    // A left-aligned title on a fully bordered block starts one column in and
    // is cut off at the far border. `label_rect` clips a zone that would fall
    // past that edge, so a hint the block never painted leaves no button
    // floating over the results grid underneath.
    let hint_row = Rect {
        x: popup.x.saturating_add(1),
        y: popup.bottom().saturating_sub(1),
        width: popup.width.saturating_sub(2),
        height: 1,
    };
    // The leading space of the bottom title.
    let stage_x: u16 = 1;
    let cancel_x = stage_x
        .saturating_add(width_of(STAGE_HINT))
        .saturating_add(width_of(COMMIT_HINT));
    hits.register(
        label_rect(hint_row, stage_x, width_of(STAGE_HINT)),
        Zone::CellEditStage,
    );
    hits.register(
        label_rect(hint_row, cancel_x, width_of(CANCEL_HINT)),
        Zone::CellEditCancel,
    );
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

    let Some(results_area) = layout.results_area else {
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

    let cell_x = results_area.x.saturating_add(1).saturating_add(x_offset);

    let row_offset = selected_row.saturating_sub(row_scroll) as u16;
    let cell_y = results_area.y.saturating_add(2).saturating_add(row_offset);

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
    use crate::app::AppState;
    use crate::ui::hit::HitMap;
    use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};

    /// The text painted inside a rect, so a test can check a zone against the
    /// pixels it claims rather than against coordinates it worked out itself.
    fn text_at(buf: &Buffer, rect: Rect) -> String {
        let mut out = String::new();
        for y in rect.y..rect.bottom() {
            for x in rect.x..rect.right() {
                if let Some(cell) = buf.cell((x, y)) {
                    out.push_str(cell.symbol());
                }
            }
        }
        out
    }

    /// Draw the popup over an 80×24 screen, on top of `under`, and hand back
    /// the painted buffer with the zones the draw registered.
    fn render(col_name: &str, original: &str, under: &[(Rect, Zone)]) -> (Buffer, HitMap, Rect) {
        let mut edit = CellEditState::new(
            0,
            0,
            col_name.to_owned(),
            original.to_owned(),
            "public".to_owned(),
            "t".to_owned(),
            vec![],
        );
        let state = AppState::new(vec![]);
        let screen = Rect::new(0, 0, 80, 24);

        let mut hits = HitMap::default();
        for (rect, zone) in under {
            hits.register(*rect, *zone);
        }

        let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("backend");
        terminal
            .draw(|frame| {
                draw(
                    frame,
                    &mut edit,
                    &state.layout,
                    &state.results,
                    frame.area(),
                    &mut hits,
                )
            })
            .expect("draw");

        let popup = compute_popup_rect(&state.layout, &state.results, screen);
        (terminal.backend().buffer().clone(), hits, popup)
    }

    /// Where a zone sits on a given screen row, if it is there at all.
    fn zone_on_row(hits: &HitMap, row: u16, want: Zone) -> Option<Rect> {
        (0..80).find_map(|x| match hits.hit(x, row) {
            Some((rect, zone)) if zone == want => Some(rect),
            _ => None,
        })
    }

    /// Each hint is a button, so its zone has to sit exactly on the word: one
    /// column out and "stage" would be "commit".
    #[test]
    fn a_hint_is_clickable_exactly_where_it_is_painted() {
        let (buffer, hits, popup) = render("email", "old", &[]);
        let row = popup.bottom() - 1;

        let stage = zone_on_row(&hits, row, Zone::CellEditStage).expect("a stage zone");
        assert_eq!(text_at(&buffer, stage), STAGE_HINT);

        let cancel = zone_on_row(&hits, row, Zone::CellEditCancel).expect("a cancel zone");
        assert_eq!(text_at(&buffer, cancel), CANCEL_HINT);
    }

    /// The hints moved to the bottom border precisely so an arbitrarily long
    /// column name can no longer clip them: the two keys that finish or abandon
    /// an edit have to stay both visible and clickable whatever the cell holds.
    #[test]
    fn a_long_column_name_no_longer_clips_the_hints() {
        let (buffer, hits, popup) = render(&"x".repeat(120), "y".repeat(120).as_str(), &[]);
        let row = popup.bottom() - 1;

        assert!(
            text_at(&buffer, popup).contains(CANCEL_HINT),
            "still painted"
        );
        let cancel = zone_on_row(&hits, row, Zone::CellEditCancel).expect("a cancel zone");
        assert_eq!(text_at(&buffer, cancel), CANCEL_HINT);
    }

    /// The popup clears what it covers, so its text area has to answer the
    /// clicks the results grid would otherwise take underneath it.
    #[test]
    fn the_text_area_takes_the_click_from_the_grid_under_it() {
        let under = [(
            Rect::new(0, 0, 80, 24),
            Zone::Panel(crate::app::FocusedPanel::Results),
        )];
        let (_, hits, popup) = render("email", "old", &under);

        assert_eq!(
            hits.hit(popup.x + 2, popup.y + 2).map(|(_, z)| z),
            Some(Zone::CellEditInput)
        );
    }

    /// A long column name pushes the hints past the right border, where the
    /// title is cut off — so there must be nothing left to click there either.
    #[test]
    fn hints_pushed_off_the_edge_register_nothing() {
        let (_, hits, popup) = render(&"x".repeat(120), "y", &[]);

        assert_eq!(zone_on_row(&hits, popup.y, Zone::CellEditStage), None);
        assert_eq!(zone_on_row(&hits, popup.y, Zone::CellEditCancel), None);
    }

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
        state.layout.results_area = Some(Rect::new(10, 10, 80, 20));
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
