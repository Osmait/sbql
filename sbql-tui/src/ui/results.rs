//! Center-bottom panel — paginated results table with sort indicators.

use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::app::{FilterBar, FocusedPanel, MutationState, ResultsState};
use crate::ui::hit::{HitMap, Side, Zone};
use crate::ui::theme;
use sbql_core::SortDirection;

// Maximum column width to prevent extremely wide cells from dominating
const MAX_COL_WIDTH: u16 = 40;
// Minimum column width
const MIN_COL_WIDTH: u16 = 6;
// Column separator spacing used by ratatui Table
const COL_SPACING: u16 = 1;

/// Values computed during draw that the caller needs to write back.
/// Geometry of the results grid for a given area.
///
/// Computed without a `Frame`, so the viewport arithmetic the scrolling and
/// paging logic depends on can be tested without rendering anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultsLayout {
    /// Width of every column, not just the visible ones.
    pub col_widths: Vec<u16>,
    /// Rows that fit below the header.
    pub viewport_height: usize,
    /// Columns that fit across, at least one even if it overflows.
    pub viewport_cols: usize,
    /// First visible column, clamped into the data.
    pub col_scroll: usize,
    /// One past the last visible column.
    pub visible_end: usize,
}

/// Everything the results panel needs to render, gathered in one place so the
/// draw call does not take nine positional arguments.
pub struct ResultsView<'a> {
    pub results: &'a ResultsState,
    pub mutation: &'a MutationState,
    pub focused: FocusedPanel,
    pub active_filter: Option<&'a str>,
    pub filter_visible: bool,
    pub spinner_frame: usize,
    pub has_active_connection: bool,
}

/// Work out the grid geometry. Pure: no rendering, no state mutation.
pub fn measure(results: &ResultsState, area: Rect) -> ResultsLayout {
    let viewport_height = (area.height.saturating_sub(3) as usize).max(1);

    if results.data.columns.is_empty() {
        return ResultsLayout {
            col_widths: Vec::new(),
            viewport_height,
            viewport_cols: 1,
            col_scroll: 0,
            visible_end: 0,
        };
    }

    let col_widths = if results.col_widths_dirty || results.cached_col_widths.is_empty() {
        compute_col_widths(&results.data.columns, &results.data.rows)
    } else {
        results.cached_col_widths.clone()
    };

    let total_cols = results.data.columns.len();
    let inner_width = area.width.saturating_sub(2) as usize;
    let col_scroll = results.col_scroll.min(total_cols.saturating_sub(1));

    let mut visible_end = col_scroll;
    let mut used_width = 0usize;
    for (ci, width) in col_widths.iter().enumerate().skip(col_scroll) {
        let w = *width as usize + COL_SPACING as usize;
        if used_width + w > inner_width && visible_end > col_scroll {
            break;
        }
        used_width += w;
        visible_end = ci + 1;
    }

    ResultsLayout {
        viewport_cols: (visible_end - col_scroll).max(1),
        col_widths,
        viewport_height,
        col_scroll,
        visible_end,
    }
}

/// Render the results grid using a layout from [`measure`].
///
/// Takes the geometry rather than computing it, so drawing has no results the
/// caller has to read back out of it.
pub fn draw(
    frame: &mut Frame,
    view: &ResultsView,
    layout: &ResultsLayout,
    area: Rect,
    hits: &mut HitMap,
) {
    let ResultsView {
        results,
        mutation,
        focused,
        active_filter,
        filter_visible,
        spinner_frame,
        has_active_connection,
    } = *view;
    let is_focused = focused == FocusedPanel::Results;

    let border_style = if is_focused {
        Style::default().fg(theme::GREEN)
    } else {
        Style::default().fg(theme::OVERLAY0)
    };

    // Build title with page info
    let page_info = if !results.data.rows.is_empty() {
        let total_shown = results.current_page * 100 + results.data.rows.len();
        if results.data.has_next_page {
            format!(
                " Results (rows 1–{total_shown}+, page {}) ",
                results.current_page + 1
            )
        } else {
            format!(" Results ({total_shown} rows) ")
        }
    } else {
        " Results ".into()
    };

    let loading_indicator = if results.is_loading {
        const SPINNER: [&str; 8] = ["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];
        format!(" {} ", SPINNER[spinner_frame % SPINNER.len()])
    } else {
        String::new()
    };

    let pending_indicator = {
        let edits = mutation.pending_edits.len();
        let deletes = mutation.pending_deletes.len();
        if edits > 0 || deletes > 0 {
            let mut parts = Vec::new();
            if edits > 0 {
                parts.push(format!("{}~", edits));
            }
            if deletes > 0 {
                parts.push(format!("{}-", deletes));
            }
            format!(" [staged: {}] ", parts.join(" "))
        } else {
            String::new()
        }
    };

    let filter_hint = if filter_visible || results.data.columns.is_empty() {
        String::new()
    } else if let Some(f) = active_filter {
        format!(" [filter: {}] / edit filter", f)
    } else {
        " / filter".to_owned()
    };

    let title = Line::from(vec![
        Span::styled(
            page_info,
            Style::default()
                .fg(theme::GREEN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(loading_indicator, Style::default().fg(theme::YELLOW)),
        Span::styled(
            pending_indicator,
            Style::default()
                .fg(theme::BASE)
                .bg(theme::YELLOW)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            filter_hint,
            if active_filter.is_some() {
                Style::default()
                    .fg(theme::MAUVE)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme::OVERLAY0)
            },
        ),
    ]);

    if results.data.columns.is_empty() {
        let msg = if results.is_loading {
            "Loading..."
        } else if !has_active_connection {
            "Connect to a database first (Enter on a connection)"
        } else {
            "No results. Run a query with Ctrl+S or F5."
        };
        let para = Paragraph::new(msg).block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(border_style),
        );
        frame.render_widget(para, area);
        return;
    }

    let all_col_widths = &layout.col_widths;
    let visible_col_count = layout.visible_end - layout.col_scroll;
    let total_cols = results.data.columns.len();
    let col_scroll = layout.col_scroll;
    let visible_end = layout.visible_end;

    let left_arrow = if col_scroll > 0 { " ◀ " } else { "" };
    let right_arrow = if visible_end < total_cols {
        " ▶ "
    } else {
        ""
    };

    // Build header row (only visible columns)
    let header_cells: Vec<Cell> = results
        .data
        .columns
        .iter()
        .enumerate()
        .skip(col_scroll)
        .take(visible_col_count)
        .map(|(i, col)| {
            let sort_indicator = sort_indicator(results.sort.as_ref(), col);
            let is_selected_col = i == results.selected_col && is_focused;
            let style = if is_selected_col {
                Style::default()
                    .fg(theme::BASE)
                    .bg(theme::BLUE)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(theme::BLUE)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            };
            Cell::from(format!("{col}{sort_indicator}")).style(style)
        })
        .collect();

    let header = Row::new(header_cells).height(1);

    let visible_rows: Vec<Row> = results
        .data
        .rows
        .iter()
        .enumerate()
        .skip(results.scroll)
        .map(|(row_idx, row)| {
            let is_selected = row_idx == results.selected_row && is_focused;
            let is_pending_delete = mutation.pending_deletes.contains_key(&row_idx);
            let cells: Vec<Cell> = row
                .iter()
                .enumerate()
                .skip(col_scroll)
                .take(visible_col_count)
                .map(|(col_idx, val)| {
                    let is_selected_cell = is_selected && col_idx == results.selected_col;
                    let is_pending_edit = mutation.pending_edits.contains_key(&(row_idx, col_idx));

                    let display_val = mutation
                        .pending_edits
                        .get(&(row_idx, col_idx))
                        .map(|e| e.new_val.as_str())
                        .unwrap_or(val.as_str());
                    let display = truncate(display_val, MAX_COL_WIDTH as usize);

                    let style = if is_pending_delete {
                        if is_selected_cell {
                            Style::default()
                                .fg(theme::TEXT)
                                .bg(theme::RED)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(theme::TEXT).bg(theme::RED)
                        }
                    } else if is_pending_edit {
                        Style::default()
                            .fg(theme::BASE)
                            .bg(theme::YELLOW)
                            .add_modifier(Modifier::BOLD)
                    } else if is_selected_cell {
                        Style::default()
                            .fg(theme::BASE)
                            .bg(theme::BLUE)
                            .add_modifier(Modifier::BOLD)
                    } else if is_selected {
                        Style::default().bg(theme::SURFACE0)
                    } else if row_idx % 2 == 0 {
                        Style::default()
                    } else {
                        Style::default().fg(theme::OVERLAY2)
                    };
                    Cell::from(display).style(style)
                })
                .collect();
            Row::new(cells).height(1)
        })
        .collect();

    let constraints: Vec<Constraint> = all_col_widths[col_scroll..visible_end]
        .iter()
        .map(|&w| Constraint::Length(w))
        .collect();

    let pending_count = mutation.pending_edits.len() + mutation.pending_deletes.len();
    let help = if is_focused {
        if pending_count > 0 {
            " ^S: stage  dd: delete  ^W: commit  Esc: discard  o: sort  /: filter "
        } else {
            " ↑↓/jk: row  ←→/hl: col  gg/G  ^d/^u  Enter: edit  dd: delete  o: sort  /: filter "
        }
    } else {
        " Tab/click: focus "
    };

    let nav_hint = if !left_arrow.is_empty() || !right_arrow.is_empty() {
        format!(
            "{left_arrow}cols {}-{} of {total_cols}{right_arrow}",
            col_scroll + 1,
            visible_end
        )
    } else {
        String::new()
    };

    // Where the markers sit inside the bottom title, measured the way the
    // renderer measures rather than by counting characters, so the regions
    // registered for them cannot disagree with where it puts the glyphs.
    let help_width = Span::raw(help).width();
    let nav_width = Span::raw(nav_hint.as_str()).width();
    let left_width = Span::raw(left_arrow).width();
    let right_width = Span::raw(right_arrow).width();

    let title_bottom = if nav_hint.is_empty() {
        Line::from(Span::styled(help, Style::default().fg(theme::OVERLAY0)))
    } else {
        Line::from(vec![
            Span::styled(help, Style::default().fg(theme::OVERLAY0)),
            Span::styled(nav_hint, Style::default().fg(theme::BLUE)),
        ])
    };

    let block = Block::default()
        .title(title)
        .title_bottom(title_bottom)
        .borders(Borders::ALL)
        .border_style(border_style);
    // Every region below hangs off the area the table will really draw into —
    // asked of the block rather than assumed, since the block owns the border.
    let inner = block.inner(area);

    // -- Hit regions --
    //
    // Taken from the rects the table itself lays out, with the same
    // constraints, spacing and flex. Adding the widths up again here is how a
    // region drifts a column away from the text it belongs to, which is the
    // failure this map exists to make impossible.
    let col_rects = Layout::horizontal(constraints.iter().copied())
        .flex(Flex::Start)
        .spacing(COL_SPACING)
        .split(Rect::new(0, 0, inner.width, 1));

    // The table's own vertical split: the header takes the first row inside the
    // border, the rest is the scrollable body.
    let header_area = Rect {
        height: inner.height.min(1),
        ..inner
    };
    let rows_area = Rect {
        y: inner.y.saturating_add(1),
        height: inner.height.saturating_sub(1),
        ..inner
    };

    for (i, col) in col_rects.iter().enumerate() {
        hits.register(
            Rect::new(
                inner.x.saturating_add(col.x),
                header_area.y,
                col.width,
                header_area.height,
            ),
            Zone::ResultsHeader(col_scroll + i),
        );
    }

    // Zipping against the body's rows stops at whichever runs out first, so a
    // pane taller than the data registers nothing below the last row.
    let first_row = first_painted_row(results, is_focused, usize::from(rows_area.height));
    for (line, (row_idx, row)) in rows_area
        .rows()
        .zip(results.data.rows.iter().enumerate().skip(first_row))
    {
        for (i, col) in col_rects.iter().enumerate() {
            let col_idx = col_scroll + i;
            // A row shorter than the header paints no cell here.
            if col_idx >= row.len() {
                break;
            }
            hits.register(
                Rect::new(inner.x.saturating_add(col.x), line.y, col.width, 1),
                Zone::ResultsCell {
                    row: row_idx,
                    col: col_idx,
                },
            );
        }
    }

    // The markers are part of the bottom title, which lands on the last row
    // inside the left border. In a pane under two rows high that is the top
    // title's row as well, and there is no saying which of them is on it.
    if area.height >= 2 {
        let bar = Rect {
            x: area.x.saturating_add(1),
            y: area.bottom().saturating_sub(1),
            width: area.width.saturating_sub(2),
            height: 1,
        };
        if !left_arrow.is_empty() {
            hits.register(
                title_slice(bar, help_width, left_width),
                Zone::ResultsColScroll(Side::Left),
            );
        }
        if !right_arrow.is_empty() {
            hits.register(
                title_slice(
                    bar,
                    (help_width + nav_width).saturating_sub(right_width),
                    right_width,
                ),
                Zone::ResultsColScroll(Side::Right),
            );
        }
    }

    let table = Table::new(visible_rows, constraints)
        .header(header)
        .block(block)
        .row_highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .column_spacing(COL_SPACING);

    let mut tbl_state = TableState::default();
    tbl_state.select(if is_focused {
        Some(results.selected_row.saturating_sub(results.scroll))
    } else {
        None
    });

    frame.render_stateful_widget(table, area, &mut tbl_state);
}

/// The first data row the grid actually paints.
///
/// Usually `results.scroll` — but the table is handed the selected row and
/// scrolls itself to keep it on screen, while `scroll` is only re-clamped when
/// the cursor moves. Shrink the pane and the two disagree until the next
/// keypress: the grid paints from further down than `scroll` says, and cells
/// registered from `scroll` would answer with a row several above the text
/// under the pointer for as long as that lasts.
fn first_painted_row(results: &ResultsState, is_focused: bool, height: usize) -> usize {
    let available = results.data.rows.len().saturating_sub(results.scroll);
    // Only a selection can push the table past its offset, and it is only given
    // one while the panel is focused.
    if !is_focused || height == 0 || available == 0 {
        return results.scroll;
    }
    let selected = results
        .selected_row
        .saturating_sub(results.scroll)
        .min(available - 1);
    if selected < available.min(height) {
        return results.scroll;
    }
    // What `Table::get_row_bounds` settles on with every row one line high:
    // scroll down until the selected row is the last one that fits.
    results.scroll + (selected + 1).saturating_sub(height)
}

/// The `width`-wide slice of the bottom title bar starting `offset` cells in.
///
/// Clipped to `bar`, because a title that does not fit is truncated at the
/// border: past the edge the marker was never painted, and the empty rect that
/// comes back is one [`HitMap::register`] drops.
fn title_slice(bar: Rect, offset: usize, width: usize) -> Rect {
    let x = bar
        .x
        .saturating_add(u16::try_from(offset).unwrap_or(u16::MAX));
    if x >= bar.right() {
        return Rect::ZERO;
    }
    let width = u16::try_from(width).unwrap_or(u16::MAX);
    Rect::new(x, bar.y, width.min(bar.right() - x), 1)
}

// ---------------------------------------------------------------------------
// Filter bar overlay (drawn over the bottom edge of the results panel)
// ---------------------------------------------------------------------------

pub fn draw_filter_bar(
    frame: &mut Frame,
    filter: &mut FilterBar,
    results_area: Rect,
    hits: &mut HitMap,
) {
    let bar_height = 3u16;
    if results_area.height < bar_height + 2 {
        return;
    }
    let bar_area = Rect {
        x: results_area.x + 1,
        y: results_area.y + results_area.height - bar_height - 1,
        width: results_area.width.saturating_sub(2),
        height: bar_height,
    };

    frame.render_widget(Clear, bar_area);

    filter.textarea.set_block(
        Block::default()
            .title(" Filter (Tab: autocomplete, Enter: apply, Esc: close) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::MAUVE)),
    );
    filter
        .textarea
        .set_cursor_style(Style::default().bg(theme::MAUVE).fg(theme::BASE));

    frame.render_widget(&filter.textarea, bar_area);

    // The typed line, inside the bar's own border. Registered so a click on the
    // filter lands on the filter rather than falling through to the grid it is
    // covering, which would move the cell cursor under an open filter bar.
    hits.register(
        Rect {
            x: bar_area.x + 1,
            y: bar_area.y + 1,
            width: bar_area.width.saturating_sub(2),
            height: 1,
        },
        Zone::FilterInput,
    );

    if filter.show_suggestions {
        let max_items = 6usize;
        let count = filter.suggestions.len().min(max_items);
        let sug_height = count as u16 + 2;
        let sug_y = bar_area.y.saturating_sub(sug_height);
        let sug_area = Rect {
            x: bar_area.x,
            y: sug_y,
            width: bar_area.width,
            height: sug_height,
        };
        frame.render_widget(Clear, sug_area);

        let mut lines = Vec::new();
        for (i, item) in filter.suggestions.iter().take(max_items).enumerate() {
            let style = if i == filter.suggestion_cursor.index() {
                Style::default().fg(theme::BASE).bg(theme::BLUE)
            } else {
                Style::default().fg(theme::TEXT)
            };
            // Registered beside the line it draws — the popup is a list, and a
            // row can never be clickable where its own text is not.
            hits.register(
                Rect {
                    x: sug_area.x + 1,
                    y: sug_area.y + 1 + u16::try_from(i).unwrap_or(u16::MAX),
                    width: sug_area.width.saturating_sub(2),
                    height: 1,
                },
                Zone::FilterSuggestion(i),
            );
            lines.push(Line::from(Span::styled(item.clone(), style)));
        }

        let title = if filter.loading_suggestions {
            " Suggestions (loading...) "
        } else {
            " Suggestions "
        };
        let sug = Paragraph::new(lines).block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme::OVERLAY0)),
        );
        frame.render_widget(sug, sug_area);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// The arrow drawn beside `col` in the header.
///
/// Reads the sort core last reported and nothing else. The header used to
/// consult a map the TUI maintained itself, which kept an arrow up after core
/// had already dropped the sort — on a disconnect, or when a connection's
/// target was edited out from under the session.
fn sort_indicator(sort: Option<&(String, SortDirection)>, col: &str) -> &'static str {
    match sort {
        Some((sorted, SortDirection::Ascending)) if sorted == col => " ▲",
        Some((sorted, SortDirection::Descending)) if sorted == col => " ▼",
        _ => "",
    }
}

fn compute_col_widths(columns: &[String], rows: &[Vec<String>]) -> Vec<u16> {
    columns
        .iter()
        .enumerate()
        .map(|(i, col)| {
            let header_w = col.len() as u16 + 2;
            let data_w = rows
                .iter()
                .take(50)
                .filter_map(|r| r.get(i))
                .map(|v| v.len() as u16)
                .max()
                .unwrap_or(0);
            (header_w.max(data_w)).clamp(MIN_COL_WIDTH, MAX_COL_WIDTH)
        })
        .collect()
}

fn truncate(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.replace('\n', "↵").replace('\r', "")
    } else {
        let truncated: String = chars[..max_chars.saturating_sub(1)].iter().collect();
        format!("{}…", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::MutationState;
    use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};
    use sbql_core::QueryResult;
    use std::collections::HashMap;

    fn state_with(columns: &[&str], rows: usize) -> ResultsState {
        let mut s = ResultsState {
            data: QueryResult {
                columns: columns.iter().map(|c| c.to_string()).collect(),
                rows: (0..rows)
                    .map(|r| columns.iter().map(|c| format!("{c}{r}")).collect())
                    .collect(),
                page: 0,
                has_next_page: false,
                total_count: None,
            },
            ..ResultsState::default()
        };
        s.col_widths_dirty = true;
        s
    }

    fn area(width: u16, height: u16) -> Rect {
        Rect {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    /// Border and header take three rows; the rest is the scrollable viewport.
    #[test]
    fn viewport_height_excludes_the_chrome() {
        let s = state_with(&["a"], 50);
        assert_eq!(measure(&s, area(80, 23)).viewport_height, 20);
    }

    /// A pane too short for any row still reports one, so paging arithmetic
    /// never divides by zero.
    #[test]
    fn viewport_height_is_never_zero() {
        let s = state_with(&["a"], 50);
        for h in 0..=3 {
            assert_eq!(measure(&s, area(80, h)).viewport_height, 1, "height {h}");
        }
    }

    #[test]
    fn an_empty_result_measures_to_nothing_visible() {
        let s = ResultsState::default();
        let l = measure(&s, area(80, 24));
        assert!(l.col_widths.is_empty());
        assert_eq!(l.visible_end, 0);
        assert_eq!(l.viewport_cols, 1);
    }

    /// Only the columns that fit are visible, and the count follows the width.
    #[test]
    fn visible_columns_follow_the_available_width() {
        let s = state_with(&["aaaa", "bbbb", "cccc", "dddd"], 3);
        let wide = measure(&s, area(200, 24));
        assert_eq!(wide.viewport_cols, 4, "all four fit in 200 columns");

        let narrow = measure(&s, area(20, 24));
        assert!(
            narrow.viewport_cols < 4,
            "expected fewer columns in a 20-wide pane, got {}",
            narrow.viewport_cols
        );
        assert_eq!(narrow.col_widths.len(), 4, "widths cover every column");
    }

    /// One column always renders even when it cannot fit, otherwise the grid
    /// would come out blank on a very narrow terminal.
    #[test]
    fn at_least_one_column_is_visible_however_narrow() {
        let s = state_with(&["a_very_long_column_name"], 3);
        assert_eq!(measure(&s, area(4, 24)).viewport_cols, 1);
    }

    /// Horizontal scroll past the last column is pulled back into range.
    #[test]
    fn column_scroll_is_clamped_into_the_data() {
        let mut s = state_with(&["a", "b", "c"], 3);
        s.col_scroll = 99;
        assert_eq!(measure(&s, area(80, 24)).col_scroll, 2);
    }

    /// Measuring is pure — the same inputs give the same answer, and nothing
    /// about the state changes.
    #[test]
    fn measuring_twice_gives_the_same_layout() {
        let s = state_with(&["a", "b"], 5);
        assert_eq!(measure(&s, area(80, 24)), measure(&s, area(80, 24)));
    }

    // -- Header sort indicator --

    #[test]
    fn the_sorted_column_gets_an_arrow() {
        let asc = ("name".to_string(), SortDirection::Ascending);
        assert_eq!(sort_indicator(Some(&asc), "name"), " ▲");
        assert_eq!(sort_indicator(Some(&asc), "id"), "");

        let desc = ("name".to_string(), SortDirection::Descending);
        assert_eq!(sort_indicator(Some(&desc), "name"), " ▼");
    }

    /// Disconnecting drops the sort in core, which reports `SortChanged(None)`.
    /// Once that lands there must be no arrow left anywhere in the header —
    /// this is the drift the separate TUI-side sort map used to leave behind.
    #[test]
    fn a_dropped_sort_leaves_no_arrow() {
        let mut s = state_with(&["id", "name"], 3);
        s.sort = Some(("name".into(), SortDirection::Ascending));

        s.sort = None; // what CoreEvent::SortChanged(None) writes

        for col in &s.data.columns {
            assert_eq!(sort_indicator(s.sort.as_ref(), col), "", "column {col}");
        }
    }

    // -- Hit regions --
    //
    // Every test here draws into a real buffer and then hit-tests a point
    // against the text painted at it. Asserting the arithmetic on its own would
    // only prove the regions agree with a second copy of the same assumptions;
    // what has to hold is that they agree with the screen.

    fn mutation() -> MutationState {
        MutationState {
            cell_edit: None,
            pending_cell_edit: None,
            pending_edits: HashMap::new(),
            pending_deletes: HashMap::new(),
            pending_delete_row: None,
            pending_d: false,
        }
    }

    /// Draw the grid for real, returning the screen and the regions it recorded.
    fn render(results: &ResultsState, w: u16, h: u16) -> (Buffer, HitMap) {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        let mutation = mutation();
        let mut hits = HitMap::default();
        let rect = area(w, h);
        let view = ResultsView {
            results,
            mutation: &mutation,
            focused: FocusedPanel::Results,
            active_filter: None,
            filter_visible: false,
            spinner_frame: 0,
            has_active_connection: true,
        };
        let layout = measure(results, rect);
        terminal
            .draw(|f| draw(f, &view, &layout, rect, &mut hits))
            .unwrap();
        (terminal.backend().buffer().clone(), hits)
    }

    /// Draw the filter bar over a results pane of the given size.
    fn render_filter(filter: &mut FilterBar, w: u16, h: u16) -> (Buffer, HitMap) {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        let mut hits = HitMap::default();
        terminal
            .draw(|f| draw_filter_bar(f, filter, area(w, h), &mut hits))
            .unwrap();
        (terminal.backend().buffer().clone(), hits)
    }

    /// The text painted across `rect`'s first row.
    fn text_at(buf: &Buffer, rect: Rect) -> String {
        (rect.x..rect.right())
            .filter_map(|x| buf.cell((x, rect.y)))
            .map(|c| c.symbol())
            .collect()
    }

    /// Where `needle` is painted, as the (x, y) of its first cell.
    fn find_text(buf: &Buffer, needle: &str) -> Option<(u16, u16)> {
        (0..buf.area.height).find_map(|y| {
            let row = text_at(buf, Rect::new(0, y, buf.area.width, 1));
            // Every symbol on screen here is one cell wide, so counting the
            // characters before the match gives its column.
            let at = row.find(needle)?;
            u16::try_from(row[..at].chars().count())
                .ok()
                .map(|x| (x, y))
        })
    }

    /// The whole screen, point by point: wherever the map answers with a cell,
    /// the text painted in that region has to be the value at those data
    /// coordinates. Returns how many points resolved, so a test can tell an
    /// agreeing map from an empty one.
    fn cells_agreeing_with_the_screen(s: &ResultsState, buf: &Buffer, hits: &HitMap) -> usize {
        let mut hit_points = 0;
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let Some((rect, Zone::ResultsCell { row, col })) = hits.hit(x, y) else {
                    continue;
                };
                let painted = text_at(buf, rect);
                assert_eq!(
                    Some(painted.trim_end()),
                    s.data.rows.get(row).and_then(|r| r.get(col)).map(|v| &**v),
                    "({x},{y}) resolves to cell {row},{col}"
                );
                hit_points += 1;
            }
        }
        hit_points
    }

    /// The point of the whole exercise: a click in the grid lands on the cell
    /// whose text is under the pointer, in data coordinates.
    #[test]
    fn a_cell_answers_with_the_coordinates_of_the_value_drawn_in_it() {
        let s = state_with(&["id", "name", "email"], 20);
        let (buf, hits) = render(&s, 80, 12);

        assert!(cells_agreeing_with_the_screen(&s, &buf, &hits) > 0);

        let (x, y) = find_text(&buf, "name3").expect("row 3's name is on screen");
        assert_eq!(
            hits.hit(x, y).map(|(_, z)| z),
            Some(Zone::ResultsCell { row: 3, col: 1 })
        );
    }

    /// Scrolling moves the data under the grid rather than the grid itself, so
    /// the regions have to follow it — in both directions at once.
    #[test]
    fn a_scrolled_cell_answers_with_its_data_coordinates_not_its_screen_ones() {
        let mut s = state_with(&["id", "name", "email"], 40);
        s.scroll = 7;
        s.col_scroll = 1;
        let (buf, hits) = render(&s, 40, 12);

        assert!(cells_agreeing_with_the_screen(&s, &buf, &hits) > 0);

        let (x, y) = find_text(&buf, "name7").expect("the first visible row");
        assert_eq!(
            hits.hit(x, y).map(|(_, z)| z),
            Some(Zone::ResultsCell { row: 7, col: 1 })
        );
        // The scrolled-off column is not drawn, so it is not clickable either.
        assert!(find_text(&buf, "id7").is_none());
    }

    /// The table scrolls itself to keep the selection on screen, and `scroll`
    /// is only re-clamped when the cursor moves — so a pane that shrank paints
    /// from further down than `scroll` says. The regions follow the pixels.
    #[test]
    fn cells_follow_the_grid_when_the_table_scrolls_itself() {
        let mut s = state_with(&["id", "name"], 40);
        s.selected_row = 20; // below a viewport this short, with scroll still 0
        let (buf, hits) = render(&s, 40, 8);

        assert!(cells_agreeing_with_the_screen(&s, &buf, &hits) > 0);

        let (x, y) = find_text(&buf, "id16").expect("the first row the table shows");
        assert_eq!(
            hits.hit(x, y).map(|(_, z)| z),
            Some(Zone::ResultsCell { row: 16, col: 0 })
        );
    }

    /// A header sorts the column under the pointer, so every header region has
    /// to sit on that column's own name.
    #[test]
    fn a_header_answers_with_the_column_whose_name_is_drawn_in_it() {
        let mut s = state_with(&["id", "name", "email"], 5);
        s.col_scroll = 1;
        let (buf, hits) = render(&s, 40, 10);

        let header_y = 1; // the first row inside the border
        let mut columns = Vec::new();
        for x in 0..buf.area.width {
            let Some((rect, Zone::ResultsHeader(c))) = hits.hit(x, header_y) else {
                continue;
            };
            let painted = text_at(&buf, rect);
            assert!(
                painted.trim_end().starts_with(&s.data.columns[c]),
                "header {c} region holds {painted:?}"
            );
            if !columns.contains(&c) {
                columns.push(c);
            }
        }
        assert_eq!(
            columns,
            vec![1, 2],
            "only the scrolled-to columns are drawn"
        );
    }

    /// The rows run out before the pane does. A click in the empty space below
    /// them must not resolve to a row that is not there.
    #[test]
    fn a_click_below_the_last_row_is_not_a_cell() {
        let s = state_with(&["id", "name"], 3);
        let (buf, hits) = render(&s, 40, 14);

        // Rows 0-2 are drawn at y=2..4; below that the grid is blank.
        let below = 6;
        assert_eq!(text_at(&buf, Rect::new(1, below, 38, 1)).trim(), "");
        for x in 0..buf.area.width {
            assert!(
                !matches!(hits.hit(x, below), Some((_, Zone::ResultsCell { .. }))),
                "({x},{below}) is below the last row"
            );
        }
    }

    /// The `◀`/`▶` markers are drawn inside the bottom title. Clickable exactly
    /// where the glyphs are, and only while they are there.
    #[test]
    fn the_column_markers_are_clickable_on_their_glyphs() {
        let columns: Vec<String> = (0..20).map(|i| format!("col{i:02}")).collect();
        let names: Vec<&str> = columns.iter().map(String::as_str).collect();
        let mut s = state_with(&names, 4);
        s.col_scroll = 1;
        let (buf, hits) = render(&s, 110, 10);

        let (x, y) = find_text(&buf, "◀").expect("a left marker");
        assert_eq!(y, 9, "the markers belong on the bottom border");
        let (rect, zone) = hits.hit(x, y).expect("a zone on the left marker");
        assert_eq!(zone, Zone::ResultsColScroll(Side::Left));
        // The region is the marker and the space either side of it, nothing
        // borrowed from the help text it follows.
        assert_eq!(text_at(&buf, rect), " ◀ ");

        let (x, y) = find_text(&buf, "▶").expect("a right marker");
        let (rect, zone) = hits.hit(x, y).expect("a zone on the right marker");
        assert_eq!(zone, Zone::ResultsColScroll(Side::Right));
        assert_eq!(text_at(&buf, rect), " ▶ ");
    }

    /// No overflow, no markers — and nothing left clickable on the row they
    /// would have been drawn on.
    #[test]
    fn no_markers_are_registered_when_every_column_fits() {
        let s = state_with(&["id", "name"], 4);
        let (buf, hits) = render(&s, 80, 10);

        assert!(find_text(&buf, "◀").is_none());
        for x in 0..buf.area.width {
            assert!(
                !matches!(hits.hit(x, 9), Some((_, Zone::ResultsColScroll(_)))),
                "({x},9) is on the bottom border"
            );
        }
    }

    /// `viewport_height` is floored at one row so the paging arithmetic never
    /// divides by zero. A pane with no room to draw a row must not inherit that
    /// fiction and register one.
    #[test]
    fn a_pane_too_short_for_a_row_registers_none() {
        let s = state_with(&["id", "name"], 5);
        for height in 1..=3 {
            let (buf, hits) = render(&s, 20, height);
            for y in 0..buf.area.height {
                for x in 0..buf.area.width {
                    assert!(
                        !matches!(hits.hit(x, y), Some((_, Zone::ResultsCell { .. }))),
                        "({x},{y}) in a pane {height} rows high"
                    );
                }
            }
        }
    }

    /// An empty grid draws a message, not a table. Nothing there is clickable,
    /// so the panel underneath keeps the click.
    #[test]
    fn an_empty_result_registers_nothing() {
        let s = ResultsState::default();
        let (buf, hits) = render(&s, 40, 10);

        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                assert_eq!(hits.hit(x, y), None, "({x},{y})");
            }
        }
    }

    fn filter_with(suggestions: &[&str]) -> FilterBar {
        FilterBar {
            visible: true,
            suggestions: suggestions.iter().map(|s| (*s).to_owned()).collect(),
            show_suggestions: !suggestions.is_empty(),
            ..FilterBar::default()
        }
    }

    /// A suggestion answers with its own index: the row under the pointer is
    /// the one that gets selected, and on a double click, applied.
    #[test]
    fn a_suggestion_row_answers_with_the_index_of_the_line_drawn_in_it() {
        let items = ["users", "orders", "payments"];
        let mut filter = filter_with(&items);
        let (buf, hits) = render_filter(&mut filter, 40, 20);

        for (i, item) in items.iter().enumerate() {
            let (x, y) = find_text(&buf, item).unwrap_or_else(|| panic!("{item} is on screen"));
            assert_eq!(
                hits.hit(x, y).map(|(_, z)| z),
                Some(Zone::FilterSuggestion(i)),
                "{item}"
            );
        }
    }

    /// The bar covers the grid, so the click has to stop at the bar.
    #[test]
    fn the_filter_line_takes_the_click_from_the_grid_it_covers() {
        let mut filter = filter_with(&[]);
        filter.textarea.insert_str("status = 'open'");
        let (buf, hits) = render_filter(&mut filter, 40, 20);

        let (x, y) = find_text(&buf, "status").expect("the typed filter");
        assert_eq!(hits.hit(x, y).map(|(_, z)| z), Some(Zone::FilterInput));
    }
}
