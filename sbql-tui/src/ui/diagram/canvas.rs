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

/// Turn the finished grid into ratatui lines, one span per run of cells that
/// share a style.
///
/// A span per cell means a heap `String` per character: a 200x60 canvas cost
/// 12 000 tiny allocations, and the cached lines are cloned again on the way
/// to the screen. Runs of one style are the norm here (borders, padding, a
/// whole column name), so coalescing collapses a row to a handful of spans.
pub(super) fn canvas_to_lines(canvas: Vec<Vec<CanvasCell>>) -> Vec<Line<'static>> {
    canvas
        .into_iter()
        .map(|row| {
            let mut spans: Vec<Span<'static>> = Vec::new();
            let mut run = String::new();
            let mut run_style: Option<Style> = None;

            for cell in row {
                match run_style {
                    // `Style` is `Copy + PartialEq`, so equal styles merge.
                    Some(style) if style == cell.style => run.push(cell.ch),
                    Some(style) => {
                        spans.push(Span::styled(std::mem::take(&mut run), style));
                        run.push(cell.ch);
                        run_style = Some(cell.style);
                    }
                    None => {
                        run.push(cell.ch);
                        run_style = Some(cell.style);
                    }
                }
            }
            if let Some(style) = run_style {
                spans.push(Span::styled(run, style));
            }
            Line::from(spans)
        })
        .collect()
}

/// Take the `width` cells starting at `x_offset`, padding on the right.
///
/// This runs on every visible row of every frame, so it walks the existing
/// spans and slices them rather than exploding the line into one span per
/// character. A span that falls wholly inside the window is moved across
/// untouched, so a horizontally unscrolled row allocates nothing at all.
pub(super) fn crop_line(line: Line<'static>, x_offset: usize, width: usize) -> Line<'static> {
    if width == 0 {
        return Line::from(Vec::<Span<'static>>::new());
    }

    let mut out: Vec<Span<'static>> = Vec::new();
    // Char index of the current span's first cell within the whole line.
    let mut pos = 0usize;
    let mut taken = 0usize;

    for sp in line.spans {
        if taken == width {
            break;
        }
        let len = sp.content.chars().count();
        if len == 0 {
            continue;
        }
        let end = pos + len;
        if end <= x_offset {
            pos = end;
            continue;
        }
        let start_char = x_offset.saturating_sub(pos);
        let take_n = (len - start_char).min(width - taken);
        if start_char == 0 && take_n == len {
            out.push(sp);
        } else {
            let style = sp.style;
            let content = slice_chars(&sp.content, start_char, take_n).to_owned();
            out.push(Span::styled(content, style));
        }
        taken += take_n;
        pos = end;
    }

    // One trailing span covers both "window runs past the end" and "line was
    // entirely to the left of the window", which is what the old per-character
    // version produced for each case too.
    if taken < width {
        out.push(Span::raw(" ".repeat(width - taken)));
    }
    Line::from(out)
}

/// Sub-slice `s` to `count` characters starting at character `start`.
///
/// Byte offsets come from `char_indices`, so the cuts always land on char
/// boundaries — the canvas is full of multi-byte box-drawing glyphs.
fn slice_chars(s: &str, start: usize, count: usize) -> &str {
    let start_byte = s.char_indices().nth(start).map_or(s.len(), |(i, _)| i);
    let rest = &s[start_byte..];
    let end_byte = rest
        .char_indices()
        .nth(count)
        .map_or(s.len(), |(i, _)| start_byte + i);
    &s[start_byte..end_byte]
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
            row_text(&c, 1).is_ascii(),
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
            !(10..20).contains(&lane),
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

    // -----------------------------------------------------------------------
    // Span coalescing
    // -----------------------------------------------------------------------

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .flat_map(|sp| sp.content.chars())
            .collect()
    }

    /// What actually reaches the terminal: a style per visible character,
    /// independent of how the characters are grouped into spans.
    fn cells(line: &Line<'_>) -> Vec<(char, Style)> {
        line.spans
            .iter()
            .flat_map(|sp| sp.content.chars().map(move |ch| (ch, sp.style)))
            .collect()
    }

    fn styled_row(cells: &[(char, Style)]) -> Vec<CanvasCell> {
        cells
            .iter()
            .map(|&(ch, style)| CanvasCell { ch, style })
            .collect()
    }

    #[test]
    fn a_uniformly_styled_row_becomes_a_single_span() {
        let s = Style::default().fg(theme::OVERLAY0);
        let row = styled_row(&[('a', s), ('b', s), ('c', s), ('d', s)]);
        let lines = canvas_to_lines(vec![row]);
        assert_eq!(
            lines[0].spans.len(),
            1,
            "one span per style run, not per cell"
        );
        assert_eq!(line_text(&lines[0]), "abcd");
    }

    #[test]
    fn a_row_breaks_into_a_span_per_style_run() {
        let a = Style::default().fg(theme::GREEN);
        let b = Style::default().fg(theme::PEACH);
        let row = styled_row(&[('x', a), ('y', a), ('z', b), ('w', a)]);
        let lines = canvas_to_lines(vec![row]);
        let spans: Vec<(&str, Style)> = lines[0]
            .spans
            .iter()
            .map(|sp| (sp.content.as_ref(), sp.style))
            .collect();
        assert_eq!(spans, vec![("xy", a), ("z", b), ("w", a)]);
    }

    #[test]
    fn an_empty_row_produces_an_empty_line() {
        let lines = canvas_to_lines(vec![Vec::new()]);
        assert_eq!(line_width(&lines[0]), 0);
    }

    /// The pre-optimisation `crop_line`: one span per character. Kept as an
    /// oracle so the coalescing version can be proved to render identically.
    fn crop_line_per_char(line: Line<'static>, x_offset: usize, width: usize) -> Line<'static> {
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

    fn assert_crops_like_the_old_version(line: &Line<'static>, x_offset: usize, width: usize) {
        let new = crop_line(line.clone(), x_offset, width);
        let old = crop_line_per_char(line.clone(), x_offset, width);
        assert_eq!(
            cells(&new),
            cells(&old),
            "crop_line({x_offset}, {width}) changed the visible characters or their styles"
        );
        assert_eq!(line_width(&new), line_width(&old));
    }

    #[test]
    fn cropping_a_multi_span_line_matches_the_per_character_version() {
        let line = Line::from(vec![
            Span::styled("abc", Style::default().fg(theme::GREEN)),
            Span::styled("de", Style::default().fg(theme::PEACH)),
            Span::raw("fghij"),
        ]);
        for x_offset in [0, 1, 3, 4, 9, 10, 25] {
            for width in [0, 1, 2, 5, 10, 30] {
                assert_crops_like_the_old_version(&line, x_offset, width);
            }
        }
    }

    /// The canvas is mostly box-drawing glyphs, which are three bytes each:
    /// cropping by byte offset would slice one in half.
    #[test]
    fn cropping_is_correct_with_multi_byte_characters() {
        let line = Line::from(vec![
            Span::styled("┌─┬─┐", Style::default().fg(theme::OVERLAY0)),
            Span::styled("región", Style::default().fg(theme::GREEN)),
            Span::raw("日本語"),
        ]);
        for x_offset in [0, 1, 2, 5, 7, 11, 13, 40] {
            for width in [0, 1, 3, 8, 20] {
                assert_crops_like_the_old_version(&line, x_offset, width);
            }
        }

        let cropped = crop_line(line, 4, 4);
        assert_eq!(line_text(&cropped), "┐reg");
    }

    /// Cropping keeps runs together instead of re-exploding them: a slice of
    /// one styled run is one span, and a run that fits entirely in the window
    /// is carried across whole.
    #[test]
    fn cropping_preserves_span_coalescing() {
        let line = Line::from(vec![Span::styled(
            "hello world",
            Style::default().fg(theme::GREEN),
        )]);

        let sliced = crop_line(line.clone(), 6, 5);
        assert_eq!(sliced.spans.len(), 1);
        assert_eq!(line_text(&sliced), "world");

        let whole = crop_line(line, 0, 11);
        assert_eq!(whole.spans.len(), 1);
        assert_eq!(line_text(&whole), "hello world");
    }
}
