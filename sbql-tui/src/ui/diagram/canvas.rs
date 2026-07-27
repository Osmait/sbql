//! A small styled-character canvas: a grid of cells you draw boxes and
//! connector lines onto, then turn into ratatui `Line`s.
//!
//! Kept apart from the diagram panel because none of it knows what a table or
//! a foreign key is. It deals in cells, rectangles, lanes and box-drawing
//! glyphs, so it can be tested without a `Frame` and reused by anything else
//! that needs to draw connected boxes in a terminal.

use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::app::DiagramGlyphMode;
use crate::ui::theme;

#[derive(Clone, Copy)]
pub(super) struct GlyphSet {
    pub(super) h: char,
    pub(super) v: char,
    pub(super) tl: char,
    pub(super) tr: char,
    pub(super) bl: char,
    pub(super) br: char,
    pub(super) cross: char,
    pub(super) ltee: char,
    pub(super) rtee: char,
    pub(super) arrow_right: char,
    pub(super) arrow_left: char,
}

pub(super) fn glyphs_for(mode: DiagramGlyphMode) -> GlyphSet {
    match mode {
        DiagramGlyphMode::Ascii => GlyphSet {
            h: '-',
            v: '|',
            tl: '+',
            tr: '+',
            bl: '+',
            br: '+',
            cross: '+',
            ltee: '+',
            rtee: '+',
            arrow_right: '>',
            arrow_left: '<',
        },
        DiagramGlyphMode::Unicode => GlyphSet {
            h: '─',
            v: '│',
            tl: '┌',
            tr: '┐',
            bl: '└',
            br: '┘',
            cross: '┼',
            ltee: '├',
            rtee: '┤',
            arrow_right: '▶',
            arrow_left: '◀',
        },
    }
}

/// A styled line is a list of Spans (to carry color information).
#[derive(Clone)]
pub(super) struct StyledLine {
    pub(super) spans: Vec<Span<'static>>,
}

#[derive(Clone)]
pub(super) struct CanvasCell {
    pub(super) ch: char,
    pub(super) style: Style,
}

impl Default for CanvasCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            style: Style::default().fg(theme::OVERLAY0),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct CanvasRect {
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) w: usize,
    pub(super) h: usize,
}

/// Cut a string to at most `max` characters.
///
/// Counts characters, not bytes: slicing by byte offset panics when the cut
/// lands inside a multi-byte character, which a table or column name with an
/// accent will do.
pub(super) fn truncate_str(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((byte_idx, _)) => &s[..byte_idx],
        None => s,
    }
}

pub(super) fn write_box(
    canvas: &mut [Vec<CanvasCell>],
    x: usize,
    y: usize,
    box_lines: &[StyledLine],
) {
    for (dy, line) in box_lines.iter().enumerate() {
        let row = y + dy;
        if row >= canvas.len() {
            break;
        }
        let mut cx = x;
        for span in &line.spans {
            for ch in span.content.chars() {
                if cx >= canvas[row].len() {
                    break;
                }
                canvas[row][cx] = CanvasCell {
                    ch,
                    style: span.style,
                };
                cx += 1;
            }
        }
    }
}

/// Find a free vertical lane that doesn't overlap with any table box rect.
/// Shifts `lane_x` to the right in increments of 2 until a free spot is found,
/// up to 20 attempts.
pub(super) fn find_free_lane(
    lane_x: usize,
    rects: &[CanvasRect],
    y_min: usize,
    y_max: usize,
    max_x: usize,
) -> usize {
    let mut x = lane_x;
    for _ in 0..20 {
        let overlaps = rects
            .iter()
            .any(|r| x >= r.x && x < r.x + r.w && y_max >= r.y && y_min < r.y + r.h);
        if !overlaps {
            return x;
        }
        x = (x + 2).min(max_x);
    }
    x
}

pub(super) fn draw_arrow(
    canvas: &mut [Vec<CanvasCell>],
    x: usize,
    y: usize,
    left_to_right: bool,
    style: Style,
    glyphs: GlyphSet,
) {
    if canvas.is_empty() || y >= canvas.len() || x >= canvas[y].len() {
        return;
    }
    let arrow = if left_to_right {
        glyphs.arrow_right
    } else {
        glyphs.arrow_left
    };
    place_line_char(&mut canvas[y][x], arrow, style, glyphs);
}

pub(super) fn draw_hline(
    canvas: &mut [Vec<CanvasCell>],
    x1: usize,
    x2: usize,
    y: usize,
    style: Style,
    glyphs: GlyphSet,
) {
    if y >= canvas.len() {
        return;
    }
    let (start, end) = if x1 <= x2 { (x1, x2) } else { (x2, x1) };
    let upper = end.min(canvas[y].len().saturating_sub(1));
    if start <= upper {
        for cell in &mut canvas[y][start..=upper] {
            place_line_char(cell, glyphs.h, style, glyphs);
        }
    }
}

pub(super) fn draw_vline(
    canvas: &mut [Vec<CanvasCell>],
    x: usize,
    y1: usize,
    y2: usize,
    style: Style,
    glyphs: GlyphSet,
) {
    if canvas.is_empty() || x >= canvas[0].len() {
        return;
    }
    let (start, end) = if y1 <= y2 { (y1, y2) } else { (y2, y1) };
    for y in start..=end {
        if y >= canvas.len() {
            break;
        }
        place_line_char(&mut canvas[y][x], glyphs.v, style, glyphs);
    }
}

pub(super) fn place_line_char(cell: &mut CanvasCell, ch: char, style: Style, glyphs: GlyphSet) {
    let merged = match (cell.ch, ch) {
        (' ', c) => c,
        (a, b) if a == glyphs.h && b == glyphs.h => glyphs.h,
        (a, b) if a == glyphs.v && b == glyphs.v => glyphs.v,
        (a, b) if (a == glyphs.h && b == glyphs.v) || (a == glyphs.v && b == glyphs.h) => {
            glyphs.cross
        }
        (_, c) if c == glyphs.h && is_vertical(cell.ch) => glyphs.cross,
        (_, c) if c == glyphs.v && is_horizontal(cell.ch) => glyphs.cross,
        (_, c) => c,
    };
    cell.ch = merged;
    cell.style = style;
}

pub(super) fn is_horizontal(ch: char) -> bool {
    matches!(
        ch,
        '-' | '+' | '─' | '┬' | '┴' | '├' | '┤' | '┼' | '┌' | '┐' | '└' | '┘'
    )
}

pub(super) fn is_vertical(ch: char) -> bool {
    matches!(
        ch,
        '|' | '+' | '│' | '┬' | '┴' | '├' | '┤' | '┼' | '┌' | '┐' | '└' | '┘'
    )
}

pub(super) fn canvas_to_lines(canvas: Vec<Vec<CanvasCell>>) -> Vec<Line<'static>> {
    canvas
        .into_iter()
        .map(|row| {
            let spans: Vec<Span<'static>> = row
                .into_iter()
                .map(|cell| Span::styled(cell.ch.to_string(), cell.style))
                .collect();
            Line::from(spans)
        })
        .collect()
}

pub(super) fn crop_line(line: Line<'static>, x_offset: usize, width: usize) -> Line<'static> {
    if width == 0 {
        return Line::from(Vec::<Span<'static>>::new());
    }

    let mut chars: Vec<Span<'static>> = Vec::new();
    for sp in line.spans {
        for ch in sp.content.chars() {
            chars.push(Span::styled(ch.to_string(), sp.style));
        }
    }

    if x_offset >= chars.len() {
        return Line::from(" ".repeat(width));
    }

    let slice: Vec<Span<'static>> = chars.into_iter().skip(x_offset).take(width).collect();

    if slice.len() < width {
        let mut padded = slice;
        padded.push(Span::raw(" ".repeat(width - padded.len())));
        Line::from(padded)
    } else {
        Line::from(slice)
    }
}

pub(super) fn line_width(line: &Line<'_>) -> usize {
    line.spans.iter().map(|sp| sp.content.chars().count()).sum()
}

// ---------------------------------------------------------------------------
// Help bar overlay at the bottom of the screen
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn blank(w: usize, h: usize) -> Vec<Vec<CanvasCell>> {
        vec![vec![CanvasCell::default(); w]; h]
    }

    fn row_text(canvas: &[Vec<CanvasCell>], y: usize) -> String {
        canvas[y].iter().map(|c| c.ch).collect()
    }

    #[test]
    fn a_horizontal_line_fills_only_its_span() {
        let mut c = blank(10, 3);
        // The span is inclusive of both ends: 2..=6 is five cells.
        draw_hline(
            &mut c,
            2,
            6,
            1,
            Style::default(),
            glyphs_for(DiagramGlyphMode::Unicode),
        );
        assert_eq!(row_text(&c, 1).trim_end(), "  ─────");
        assert_eq!(row_text(&c, 0).trim(), "", "other rows untouched");
    }

    #[test]
    fn a_vertical_line_fills_only_its_span() {
        let mut c = blank(4, 6);
        draw_vline(
            &mut c,
            1,
            1,
            4,
            Style::default(),
            glyphs_for(DiagramGlyphMode::Unicode),
        );
        let col: String = (0..6).map(|y| c[y][1].ch).collect();
        assert_eq!(col, " ││││ ");
    }

    /// Where a horizontal and a vertical line meet, the glyph becomes a
    /// junction rather than one overwriting the other.
    #[test]
    fn crossing_lines_join_instead_of_overwriting() {
        let g = glyphs_for(DiagramGlyphMode::Unicode);
        let mut c = blank(10, 5);
        draw_hline(&mut c, 0, 9, 2, Style::default(), g);
        draw_vline(&mut c, 5, 0, 5, Style::default(), g);
        let joint = c[2][5].ch;
        assert!(
            !is_horizontal(joint) || !is_vertical(joint) || joint == '┼',
            "expected a junction glyph at the crossing, got {joint:?}"
        );
        assert_ne!(joint, ' ');
    }

    #[test]
    fn drawing_outside_the_canvas_is_ignored() {
        let g = glyphs_for(DiagramGlyphMode::Unicode);
        let mut c = blank(4, 2);
        draw_hline(&mut c, 0, 100, 99, Style::default(), g);
        draw_vline(&mut c, 99, 0, 100, Style::default(), g);
        draw_arrow(&mut c, 99, 99, true, Style::default(), g);
        assert_eq!(
            row_text(&c, 0).trim(),
            "",
            "nothing should have been written"
        );
    }

    #[test]
    fn ascii_mode_uses_no_box_drawing_characters() {
        let g = glyphs_for(DiagramGlyphMode::Ascii);
        let mut c = blank(8, 3);
        draw_hline(&mut c, 0, 5, 1, Style::default(), g);
        assert!(
            row_text(&c, 1).chars().all(|ch| ch.is_ascii()),
            "ascii mode must stay ascii: {:?}",
            row_text(&c, 1)
        );
    }

    /// Byte slicing used to panic here: cutting "héllo" at 2 lands inside the
    /// two-byte 'é'. Any table or column name with an accent could crash the
    /// diagram.
    #[test]
    fn truncating_counts_characters_not_bytes() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello", 3), "hel");
        assert_eq!(truncate_str("héllo", 2), "hé");
        assert_eq!(truncate_str("héllo", 3), "hél");
        assert_eq!(truncate_str("región", 4), "regi");
        assert_eq!(truncate_str("日本語テーブル", 3), "日本語");
    }

    #[test]
    fn a_lane_is_free_only_where_nothing_blocks_it() {
        let rects = [CanvasRect {
            x: 10,
            y: 0,
            w: 10,
            h: 5,
        }];
        let lane = find_free_lane(8, &rects, 0, 5, 100);
        assert!(
            lane < 10 || lane >= 20,
            "lane {lane} runs straight through the box"
        );
    }

    /// Cropping always returns exactly `width` cells, padding with spaces, so
    /// rows stay aligned when the canvas is scrolled.
    #[test]
    fn cropping_a_line_always_fills_the_requested_width() {
        let line = Line::from(vec![Span::raw("abcdefghij")]);
        assert_eq!(line_width(&crop_line(line.clone(), 0, 4)), 4);
        assert_eq!(
            line_width(&crop_line(line.clone(), 8, 10)),
            10,
            "padded at the end"
        );
        assert_eq!(
            line_width(&crop_line(line.clone(), 20, 5)),
            5,
            "past the end is blank"
        );
        assert_eq!(line_width(&crop_line(line, 0, 0)), 0);
    }
}
