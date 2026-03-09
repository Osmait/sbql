//! Center-bottom panel — paginated results table with sort indicators.

use ratatui::{
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::app::{FilterBar, FocusedPanel, MutationState, ResultsState};
use crate::ui::theme;
use sbql_core::SortDirection;

// Maximum column width to prevent extremely wide cells from dominating
const MAX_COL_WIDTH: u16 = 40;
// Minimum column width
const MIN_COL_WIDTH: u16 = 6;
// Column separator spacing used by ratatui Table
const COL_SPACING: u16 = 1;

/// Values computed during draw that the caller needs to write back.
pub struct DrawOutput {
    pub col_widths: Vec<u16>,
    pub viewport_height: usize,
    pub viewport_cols: usize,
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    frame: &mut Frame,
    results: &ResultsState,
    mutation: &MutationState,
    focused: FocusedPanel,
    active_filter: Option<&str>,
    filter_visible: bool,
    spinner_frame: usize,
    has_active_connection: bool,
    area: Rect,
) -> DrawOutput {
    let is_focused = focused == FocusedPanel::Results;

    let viewport_height = area.height.saturating_sub(3) as usize;
    let viewport_height = viewport_height.max(1);

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
        return DrawOutput {
            col_widths: Vec::new(),
            viewport_height,
            viewport_cols: 1,
        };
    }

    // --- Compute column widths for ALL columns (cached when data hasn't changed) ---
    let all_col_widths = if results.col_widths_dirty || results.cached_col_widths.is_empty() {
        compute_col_widths(&results.data.columns, &results.data.rows)
    } else {
        results.cached_col_widths.clone()
    };
    let total_cols = results.data.columns.len();

    let inner_width = area.width.saturating_sub(2) as usize;

    let col_scroll = results.col_scroll.min(total_cols.saturating_sub(1));
    let mut visible_end = col_scroll;
    let mut used_width = 0usize;
    #[allow(clippy::needless_range_loop)]
    for ci in col_scroll..total_cols {
        let w = all_col_widths[ci] as usize + COL_SPACING as usize;
        if used_width + w > inner_width && visible_end > col_scroll {
            break;
        }
        used_width += w;
        visible_end = ci + 1;
    }
    let visible_col_count = visible_end - col_scroll;
    let viewport_cols = visible_col_count.max(1);

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
            let sort_indicator = match results.sort_state.get(col) {
                Some(SortDirection::Ascending) => " ▲",
                Some(SortDirection::Descending) => " ▼",
                None => "",
            };
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

    let title_bottom = if nav_hint.is_empty() {
        Line::from(Span::styled(help, Style::default().fg(theme::OVERLAY0)))
    } else {
        Line::from(vec![
            Span::styled(help, Style::default().fg(theme::OVERLAY0)),
            Span::styled(nav_hint, Style::default().fg(theme::BLUE)),
        ])
    };

    let table = Table::new(visible_rows, constraints)
        .header(header)
        .block(
            Block::default()
                .title(title)
                .title_bottom(title_bottom)
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .column_spacing(COL_SPACING);

    let mut tbl_state = TableState::default();
    tbl_state.select(if is_focused {
        Some(results.selected_row.saturating_sub(results.scroll))
    } else {
        None
    });

    frame.render_stateful_widget(table, area, &mut tbl_state);

    DrawOutput {
        col_widths: all_col_widths,
        viewport_height,
        viewport_cols,
    }
}

// ---------------------------------------------------------------------------
// Filter bar overlay (drawn over the bottom edge of the results panel)
// ---------------------------------------------------------------------------

pub fn draw_filter_bar(frame: &mut Frame, filter: &mut FilterBar, results_area: Rect) {
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
            let style = if i == filter.selected_suggestion {
                Style::default().fg(theme::BASE).bg(theme::BLUE)
            } else {
                Style::default().fg(theme::TEXT)
            };
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
