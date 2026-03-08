//! Center-top panel — SQL editor with tree-sitter syntax highlighting
//! and autocomplete popup.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{ConnectionState, EditorMode, EditorState, FocusedPanel};
use crate::completion::CompletionKind;
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

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    // --- Build highlighted lines if cache is empty ---
    let source = editor.sql();
    if editor.highlight_cache.is_none() {
        editor.highlight_cache = Some(editor.highlighter.highlight_lines(&source));
    }
    let hl_lines = editor.highlight_cache.as_ref().unwrap();

    // --- Cursor position ---
    let (cursor_row, cursor_col) = editor.textarea.cursor();

    // --- Scroll adjustment ---
    let vp_h = inner.height as usize;
    let vp_w = inner.width as usize;

    // Keep cursor visible vertically
    if cursor_row < editor.scroll_row {
        editor.scroll_row = cursor_row;
    } else if cursor_row >= editor.scroll_row + vp_h {
        editor.scroll_row = cursor_row.saturating_sub(vp_h - 1);
    }
    // Keep cursor visible horizontally
    if cursor_col < editor.scroll_col {
        editor.scroll_col = cursor_col;
    } else if cursor_col >= editor.scroll_col + vp_w {
        editor.scroll_col = cursor_col.saturating_sub(vp_w - 1);
    }

    // --- Cursor style ---
    let cursor_style = if is_focused {
        match editor.mode {
            EditorMode::Normal => Style::default().bg(theme::MAUVE).fg(theme::BASE),
            EditorMode::Insert => Style::default().bg(theme::YELLOW).fg(theme::BASE),
        }
    } else {
        Style::default()
    };

    let cursor_line_bg = if is_focused {
        Some(theme::SURFACE0)
    } else {
        None
    };

    // --- Build visible lines ---
    let mut visible_lines: Vec<Line> = Vec::with_capacity(vp_h);
    let text_lines: Vec<String> = editor.textarea.lines().iter().map(|s| s.to_string()).collect();

    for view_row in 0..vp_h {
        let line_idx = editor.scroll_row + view_row;

        if line_idx >= hl_lines.len() {
            // Past end of document — render empty line
            visible_lines.push(Line::from(Span::raw("")));
            continue;
        }

        let segments = &hl_lines[line_idx];
        let is_cursor_line = line_idx == cursor_row && is_focused;

        // Flatten segments into a list of (style, char) pairs for easy column slicing
        let raw_line = text_lines.get(line_idx).map(|s| s.as_str()).unwrap_or("");
        let line_chars: Vec<char> = raw_line.chars().collect();
        let total_cols = line_chars.len();

        // Build per-char style array from segments
        let mut char_styles: Vec<Style> = vec![Style::default().fg(theme::TEXT); total_cols];
        {
            let mut col = 0;
            for (style, text) in segments {
                for _ in text.chars() {
                    if col < total_cols {
                        char_styles[col] = *style;
                    }
                    col += 1;
                }
            }
        }

        // Apply cursor-line background
        if is_cursor_line {
            if let Some(bg) = cursor_line_bg {
                for s in char_styles.iter_mut() {
                    *s = s.bg(bg);
                }
            }
        }

        // Apply cursor style at cursor position
        if is_cursor_line && cursor_col < total_cols {
            char_styles[cursor_col] = cursor_style;
        }

        // Slice to viewport columns
        let col_start = editor.scroll_col.min(total_cols);
        let col_end = (editor.scroll_col + vp_w).min(total_cols);

        let mut spans: Vec<Span> = Vec::new();

        if col_start < col_end {
            // Group consecutive chars with the same style into spans
            let mut run_start = col_start;
            let mut run_style = char_styles[col_start];

            for c in (col_start + 1)..=col_end {
                let at_end = c == col_end;
                let style_changed = !at_end && char_styles[c] != run_style;

                if at_end || style_changed {
                    let text: String = line_chars[run_start..c].iter().collect();
                    spans.push(Span::styled(text, run_style));
                    if !at_end {
                        run_start = c;
                        run_style = char_styles[c];
                    }
                }
            }
        }

        // If the cursor is at the end of the line (past last char), show cursor block
        if is_cursor_line && cursor_col >= total_cols && cursor_col >= col_start && cursor_col < col_start + vp_w {
            // Pad with spaces to reach cursor
            let rendered_cols = col_end.saturating_sub(col_start);
            let cursor_vp_col = cursor_col - col_start;
            if cursor_vp_col > rendered_cols {
                let pad = " ".repeat(cursor_vp_col - rendered_cols);
                let bg_style = if let Some(bg) = cursor_line_bg {
                    Style::default().bg(bg)
                } else {
                    Style::default()
                };
                spans.push(Span::styled(pad, bg_style));
            }
            spans.push(Span::styled(" ", cursor_style));
        }

        visible_lines.push(Line::from(spans));
    }

    let paragraph = Paragraph::new(visible_lines);
    frame.render_widget(paragraph, inner);

    // --- Completion popup ---
    if editor.completion.visible && !editor.completion.items.is_empty() && is_focused {
        draw_completion_popup(frame, editor, inner, cursor_row, cursor_col);
    }
}

fn draw_completion_popup(
    frame: &mut Frame,
    editor: &EditorState,
    editor_inner: Rect,
    cursor_row: usize,
    cursor_col: usize,
) {
    let comp = &editor.completion;
    let item_count = comp.items.len().min(10);
    let popup_height = item_count as u16 + 2; // +2 for borders
    let popup_width = 40u16.min(editor_inner.width);

    // Position below cursor
    let cursor_vp_row = cursor_row.saturating_sub(editor.scroll_row) as u16;
    let cursor_vp_col = cursor_col.saturating_sub(editor.scroll_col) as u16;

    let popup_x = editor_inner.x + cursor_vp_col.min(editor_inner.width.saturating_sub(popup_width));

    // Try below cursor first, then above
    let below_y = editor_inner.y + cursor_vp_row + 1;
    let space_below = editor_inner.bottom().saturating_sub(below_y);
    let (popup_y, actual_height) = if space_below >= popup_height {
        (below_y, popup_height)
    } else {
        // Try above
        let above_y = editor_inner.y + cursor_vp_row;
        if above_y >= popup_height {
            (above_y - popup_height, popup_height)
        } else {
            // Use whatever space is available below
            (below_y, space_below.max(3))
        }
    };

    let popup_rect = Rect::new(popup_x, popup_y, popup_width, actual_height);

    // Build popup content
    let popup_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::OVERLAY0));

    let popup_inner = popup_block.inner(popup_rect);
    frame.render_widget(Clear, popup_rect);
    frame.render_widget(popup_block, popup_rect);

    let max_visible = popup_inner.height as usize;
    let mut lines: Vec<Line> = Vec::with_capacity(max_visible);

    for (i, item) in comp.items.iter().enumerate().take(max_visible) {
        let is_selected = i == comp.selected;
        let icon = match item.kind {
            CompletionKind::Table => Span::styled("T ", Style::default().fg(theme::BLUE)),
            CompletionKind::Column => Span::styled("C ", Style::default().fg(theme::YELLOW)),
            CompletionKind::Keyword => Span::styled("K ", Style::default().fg(theme::OVERLAY1)),
        };

        let text_style = if is_selected {
            Style::default().fg(theme::TEXT).bg(theme::BLUE)
        } else {
            Style::default().fg(theme::TEXT)
        };

        let detail = if item.detail.is_empty() {
            Span::raw("")
        } else {
            Span::styled(
                format!("  {}", item.detail),
                if is_selected {
                    Style::default().fg(theme::SUBTEXT0).bg(theme::BLUE)
                } else {
                    Style::default().fg(theme::OVERLAY0)
                },
            )
        };

        let icon_styled = if is_selected {
            Span::styled(icon.content.to_string(), icon.style.bg(theme::BLUE))
        } else {
            icon
        };

        lines.push(Line::from(vec![
            icon_styled,
            Span::styled(&item.text, text_style),
            detail,
        ]));
    }

    let popup_paragraph = Paragraph::new(lines);
    frame.render_widget(popup_paragraph, popup_inner);
}
