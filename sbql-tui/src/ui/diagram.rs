//! Full-screen database diagram renderer.
//!
//! Layout:
//!   ┌─ Table list (25%) ──────┬─ Diagram canvas (75%) ────────────────────┐
//!   │  public.users           │  ┌─ public.users ──────────────────┐      │
//!   │  public.posts           │  │ * id          integer           │      │
//!   │  ...                    │  │   name        varchar           │      │
//!   │                         │  └────────────────────────────────┘      │
//!   └─────────────────────────┴────────────────────────────────────────────┘
//!   [ hjkl/arrows scroll canvas | Tab/j/k navigate list | Esc/q exits ]

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use sbql_core::TableSchema;

use crate::app::{DiagramGlyphMode, DiagramState};
use crate::ui::theme;

// ---------------------------------------------------------------------------
// Box layout constants
// ---------------------------------------------------------------------------

/// Vertical gap between rows of table boxes.
const V_GAP: u16 = 2;
/// Number of table columns in the canvas layout.
const COLS_PER_ROW: u16 = 3;
/// Fixed width of each table box (inner content width = BOX_WIDTH - 2 borders).
const BOX_WIDTH: u16 = 36;

// Public re-exports used by main.rs for jump_canvas_to_table
/// Approximate height of one row of table boxes (columns vary, use conservative estimate).
pub const BOX_ROW_HEIGHT: u16 = 12;
pub const COLS_PER_ROW_PUB: usize = COLS_PER_ROW as usize;
pub const V_GAP_PUB: u16 = V_GAP;

#[derive(Clone, Copy)]
struct GlyphSet {
    h: char,
    v: char,
    tl: char,
    tr: char,
    bl: char,
    br: char,
    cross: char,
}

fn glyphs_for(mode: DiagramGlyphMode) -> GlyphSet {
    match mode {
        DiagramGlyphMode::Ascii => GlyphSet {
            h: '-',
            v: '|',
            tl: '+',
            tr: '+',
            bl: '+',
            br: '+',
            cross: '+',
        },
        DiagramGlyphMode::Unicode => GlyphSet {
            h: '─',
            v: '│',
            tl: '┌',
            tr: '┐',
            bl: '└',
            br: '┘',
            cross: '┼',
        },
    }
}

// ---------------------------------------------------------------------------
// Public draw entry point
// ---------------------------------------------------------------------------

pub fn draw(frame: &mut Frame, state: &mut DiagramState) {
    let full = frame.area();

    // Split into left sidebar + right canvas
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(22), Constraint::Percentage(78)])
        .split(full);

    draw_sidebar(frame, state, split[0]);
    draw_canvas(frame, state, split[1]);
    draw_help_bar(frame, full, state.focus_mode);
}

fn visible_table_indices(state: &DiagramState) -> Vec<usize> {
    let tables = &state.data.tables;
    if tables.is_empty() {
        return Vec::new();
    }
    if !state.focus_mode {
        return (0..tables.len()).collect();
    }

    let selected = state.selected_table.min(tables.len().saturating_sub(1));
    let selected_key = tables[selected].qualified();
    let mut keys = std::collections::HashSet::new();
    keys.insert(selected_key.clone());

    for fk in &state.data.foreign_keys {
        let from_key = format!("{}.{}", fk.from_schema, fk.from_table);
        let to_key = format!("{}.{}", fk.to_schema, fk.to_table);
        if from_key == selected_key {
            keys.insert(to_key);
        } else if to_key == selected_key {
            keys.insert(from_key);
        }
    }

    tables
        .iter()
        .enumerate()
        .filter_map(|(idx, t)| keys.contains(&t.qualified()).then_some(idx))
        .collect()
}

fn visible_foreign_keys<'a>(
    state: &'a DiagramState,
    visible_keys: &std::collections::HashSet<String>,
) -> Vec<&'a sbql_core::ForeignKey> {
    if !state.focus_mode {
        return state.data.foreign_keys.iter().collect();
    }

    state
        .data
        .foreign_keys
        .iter()
        .filter(|fk| {
            let from_key = format!("{}.{}", fk.from_schema, fk.from_table);
            let to_key = format!("{}.{}", fk.to_schema, fk.to_table);
            visible_keys.contains(&from_key) && visible_keys.contains(&to_key)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Left sidebar: table list
// ---------------------------------------------------------------------------

fn draw_sidebar(frame: &mut Frame, state: &DiagramState, area: Rect) {
    let visible_indices = visible_table_indices(state);
    let tables = &state.data.tables;
    let items: Vec<ListItem> = visible_indices
        .iter()
        .filter_map(|&idx| tables.get(idx))
        .map(|t| ListItem::new(t.qualified()))
        .collect();
    let item_count = items.len();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Tables (Tab/j/k) "),
        )
        .highlight_style(
            Style::default()
                .fg(theme::BLUE)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    let mut list_state = ListState::default();
    let selected_visible = visible_indices
        .iter()
        .position(|&idx| idx == state.selected_table);
    list_state.select(if item_count == 0 {
        None
    } else {
        Some(selected_visible.unwrap_or(0))
    });

    frame.render_stateful_widget(list, area, &mut list_state);
}

// ---------------------------------------------------------------------------
// Right canvas: ASCII table boxes
// ---------------------------------------------------------------------------

fn draw_canvas(frame: &mut Frame, state: &mut DiagramState, area: Rect) {
    let data = &state.data;
    let visible_indices = visible_table_indices(state);
    let visible_keys: std::collections::HashSet<String> = visible_indices
        .iter()
        .filter_map(|&i| data.tables.get(i))
        .map(|t| t.qualified())
        .collect();
    let visible_fk_count = visible_foreign_keys(state, &visible_keys).len();

    // Collect the lines that make up the full virtual canvas
    let lines = build_canvas_lines(state, area.width);

    let canvas_height = lines.len();
    let canvas_width = lines.iter().map(line_width).max().unwrap_or(0);
    let inner_h = area.height.saturating_sub(2) as usize;
    let inner_w = area.width.saturating_sub(2) as usize;

    let max_scroll_y = canvas_height.saturating_sub(inner_h) as u16;
    let max_scroll_x = canvas_width.saturating_sub(inner_w) as u16;
    state.scroll_y = state.scroll_y.min(max_scroll_y);
    state.scroll_x = state.scroll_x.min(max_scroll_x);

    // Apply scroll (vertical + horizontal)
    let scrolled: Vec<Line> = lines
        .into_iter()
        .skip(state.scroll_y as usize)
        .map(|line| crop_line(line, state.scroll_x as usize, inner_w))
        .take(area.height.saturating_sub(2) as usize) // leave room for border
        .collect();

    let paragraph = Paragraph::new(scrolled)
        .block(Block::default().borders(Borders::ALL).title(format!(
            " Diagram ({} tables, {} FKs, mode: {}, glyph: {}) — hjkl to scroll ",
            visible_indices.len(),
            visible_fk_count,
            if state.focus_mode { "focus" } else { "all" },
            if state.glyph_mode == DiagramGlyphMode::Ascii {
                "ascii"
            } else {
                "unicode"
            }
        )))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

// ---------------------------------------------------------------------------
// Canvas line builder
// ---------------------------------------------------------------------------

/// Build the full virtual canvas as a vector of ratatui `Line`s.
/// Tables are laid out in a grid of COLS_PER_ROW columns, scrollable via
/// scroll_x / scroll_y.
fn build_canvas_lines(state: &DiagramState, _canvas_width: u16) -> Vec<Line<'static>> {
    let tables = &state.data.tables;
    let glyphs = glyphs_for(state.glyph_mode);

    let visible_indices = visible_table_indices(state);
    let visible_keys: std::collections::HashSet<String> = visible_indices
        .iter()
        .filter_map(|&i| tables.get(i))
        .map(|t| t.qualified())
        .collect();
    let visible_fks = visible_foreign_keys(state, &visible_keys);
    let visible_tables: Vec<&TableSchema> = visible_indices
        .iter()
        .filter_map(|&i| tables.get(i))
        .collect();

    if visible_tables.is_empty() {
        return vec![Line::from(Span::styled(
            "  No tables found in the current database.",
            Style::default().fg(theme::OVERLAY0),
        ))];
    }

    let selected_idx = state.selected_table;
    let selected_table = tables.get(selected_idx);
    let selected_key = selected_table.map(|t| t.qualified());

    // For each table, compute which indices are FK source columns (point to another table)
    // and which are FK target columns (referenced by another table).
    // We store this as a set of column names per table.
    use std::collections::{HashMap, HashSet};
    let mut fk_from: HashMap<String, HashSet<String>> = HashMap::new(); // qualified table → set of from_col names
    let mut fk_to: HashMap<String, HashSet<String>> = HashMap::new(); // qualified table → set of to_col names
    for fk in &visible_fks {
        let from_key = format!("{}.{}", fk.from_schema, fk.from_table);
        let to_key = format!("{}.{}", fk.to_schema, fk.to_table);
        fk_from
            .entry(from_key)
            .or_default()
            .insert(fk.from_col.clone());
        fk_to.entry(to_key).or_default().insert(fk.to_col.clone());
    }

    // Tables related to selected (either FK from or FK to)
    let related_tables: HashSet<String> = if let Some(sel) = selected_table {
        let sel_key = sel.qualified();
        visible_fks
            .iter()
            .filter_map(|fk| {
                let from_key = format!("{}.{}", fk.from_schema, fk.from_table);
                let to_key = format!("{}.{}", fk.to_schema, fk.to_table);
                if from_key == sel_key {
                    Some(to_key)
                } else if to_key == sel_key {
                    Some(from_key)
                } else {
                    None
                }
            })
            .collect()
    } else {
        HashSet::new()
    };

    // Hierarchical layout (graph-like) for both all/focus modes.
    const LAYER_X_GAP: usize = 14;
    const NODE_Y_GAP: usize = 2;
    const COMPONENT_GAP: usize = 4;

    let mut key_to_visible: HashMap<String, usize> = HashMap::new();
    for (vidx, t) in visible_tables.iter().enumerate() {
        key_to_visible.insert(t.qualified(), vidx);
    }

    let mut undirected_adj: Vec<Vec<usize>> = vec![Vec::new(); visible_tables.len()];
    let mut indegree: Vec<usize> = vec![0; visible_tables.len()];
    for fk in &visible_fks {
        let from_key = format!("{}.{}", fk.from_schema, fk.from_table);
        let to_key = format!("{}.{}", fk.to_schema, fk.to_table);
        let (Some(&a), Some(&b)) = (key_to_visible.get(&from_key), key_to_visible.get(&to_key))
        else {
            continue;
        };
        if !undirected_adj[a].contains(&b) {
            undirected_adj[a].push(b);
        }
        if !undirected_adj[b].contains(&a) {
            undirected_adj[b].push(a);
        }
        indegree[b] = indegree[b].saturating_add(1);
    }

    let selected_visible_idx = visible_indices
        .iter()
        .position(|&gidx| gidx == selected_idx);

    let mut positions: Vec<(usize, usize)> = vec![(0, 0); visible_tables.len()];
    let mut seen = vec![false; visible_tables.len()];
    let mut component_starts: Vec<usize> = (0..visible_tables.len()).collect();
    component_starts.sort_by_key(|&i| visible_tables[i].qualified());
    if let Some(sel) = selected_visible_idx {
        component_starts.retain(|&i| i != sel);
        component_starts.insert(0, sel);
    }

    let mut comp_y = 0usize;
    for start in component_starts {
        if seen[start] {
            continue;
        }

        let mut stack = vec![start];
        let mut component_nodes = Vec::new();
        seen[start] = true;
        while let Some(n) = stack.pop() {
            component_nodes.push(n);
            for &nb in &undirected_adj[n] {
                if !seen[nb] {
                    seen[nb] = true;
                    stack.push(nb);
                }
            }
        }

        let root = if let Some(sel) = selected_visible_idx {
            if component_nodes.contains(&sel) {
                sel
            } else {
                *component_nodes
                    .iter()
                    .min_by_key(|&&n| (indegree[n], visible_tables[n].qualified()))
                    .unwrap_or(&component_nodes[0])
            }
        } else {
            *component_nodes
                .iter()
                .min_by_key(|&&n| (indegree[n], visible_tables[n].qualified()))
                .unwrap_or(&component_nodes[0])
        };

        let mut level: Vec<Option<usize>> = vec![None; visible_tables.len()];
        let mut q = std::collections::VecDeque::new();
        level[root] = Some(0);
        q.push_back(root);
        while let Some(n) = q.pop_front() {
            let base = level[n].unwrap_or(0);
            for &nb in &undirected_adj[n] {
                if level[nb].is_none() {
                    level[nb] = Some(base + 1);
                    q.push_back(nb);
                }
            }
        }

        let max_layer = component_nodes
            .iter()
            .map(|&n| level[n].unwrap_or(0))
            .max()
            .unwrap_or(0);
        let mut layers: Vec<Vec<usize>> = vec![Vec::new(); max_layer + 1];
        for &n in &component_nodes {
            layers[level[n].unwrap_or(0)].push(n);
        }
        for layer_nodes in &mut layers {
            layer_nodes.sort_by_key(|&n| visible_tables[n].qualified());
        }

        let row_stride = component_nodes
            .iter()
            .map(|&n| table_box_height(visible_tables[n]))
            .max()
            .unwrap_or(3)
            + NODE_Y_GAP;
        let max_rows = layers.iter().map(Vec::len).max().unwrap_or(1);

        for (lx, layer_nodes) in layers.iter().enumerate() {
            for (ly, &node) in layer_nodes.iter().enumerate() {
                positions[node] = (
                    lx * (BOX_WIDTH as usize + LAYER_X_GAP),
                    comp_y + ly * row_stride,
                );
            }
        }

        comp_y += max_rows * row_stride + COMPONENT_GAP;
    }

    let mut rects: HashMap<String, CanvasRect> = HashMap::new();
    let mut table_idx_by_key: HashMap<String, usize> = HashMap::new();
    let mut boxes_to_draw: Vec<(usize, usize, Vec<StyledLine>)> = Vec::new();
    let mut canvas_w = 1usize;
    let mut canvas_h = 1usize;

    for (vidx, t) in visible_tables.iter().enumerate() {
        let global_idx = visible_indices[vidx];
        let is_selected = global_idx == selected_idx;
        let is_related = related_tables.contains(&t.qualified());
        let empty_set = HashSet::new();
        let from_cols = fk_from.get(&t.qualified()).unwrap_or(&empty_set);
        let to_cols = fk_to.get(&t.qualified()).unwrap_or(&empty_set);
        let box_lines = render_table_box(t, is_selected, is_related, from_cols, to_cols, glyphs);
        let (x, y) = positions[vidx];

        canvas_w = canvas_w.max(x + BOX_WIDTH as usize + 3);
        canvas_h = canvas_h.max(y + box_lines.len() + 2);

        boxes_to_draw.push((x, y, box_lines.clone()));

        let key = t.qualified();
        rects.insert(
            key.clone(),
            CanvasRect {
                x,
                y,
                w: BOX_WIDTH as usize,
                h: box_lines.len(),
            },
        );
        table_idx_by_key.insert(key, global_idx);
    }

    let mut canvas = vec![vec![CanvasCell::default(); canvas_w.max(1)]; canvas_h.max(1)];

    // Draw FK connectors first (underlay), then table boxes on top so
    // connectors do not reduce text readability inside table boxes.
    let mut edges: Vec<PreparedEdge> = Vec::new();
    for fk in &visible_fks {
        let from_key = format!("{}.{}", fk.from_schema, fk.from_table);
        let to_key = format!("{}.{}", fk.to_schema, fk.to_table);

        let (Some(from_rect), Some(to_rect)) = (rects.get(&from_key), rects.get(&to_key)) else {
            continue;
        };
        let (Some(&from_idx), Some(&to_idx)) = (
            table_idx_by_key.get(&from_key),
            table_idx_by_key.get(&to_key),
        ) else {
            continue;
        };

        let sy = endpoint_y(&tables[from_idx], *from_rect, &fk.from_col);
        let ty = endpoint_y(&tables[to_idx], *to_rect, &fk.to_col);

        let highlighted = selected_key
            .as_ref()
            .map(|s| s == &from_key || s == &to_key)
            .unwrap_or(false);
        let style = if highlighted {
            Style::default()
                .fg(theme::YELLOW)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::SURFACE2)
        };

        let from_center = from_rect.x + (from_rect.w / 2);
        let to_center = to_rect.x + (to_rect.w / 2);
        let left_to_right = from_center <= to_center;
        let sx = if left_to_right {
            from_rect.x + from_rect.w
        } else {
            from_rect.x.saturating_sub(1)
        };
        let tx = if left_to_right {
            to_rect.x.saturating_sub(1)
        } else {
            to_rect.x + to_rect.w
        };

        edges.push(PreparedEdge {
            from_idx,
            to_idx,
            sy,
            ty,
            sx,
            tx,
            left_to_right,
            highlighted,
            style,
        });
    }

    // Bundle edges by table pair so multiple FKs share a trunk line.
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<(usize, usize), Vec<PreparedEdge>> = BTreeMap::new();
    for edge in edges {
        let key = if edge.from_idx <= edge.to_idx {
            (edge.from_idx, edge.to_idx)
        } else {
            (edge.to_idx, edge.from_idx)
        };
        groups.entry(key).or_default().push(edge);
    }

    let max_x = canvas[0].len().saturating_sub(1);
    const LANE_BUCKET_WIDTH: usize = 12;
    const LANE_BUCKET_SLOTS: usize = 3;
    let mut lane_bucket_counts: HashMap<usize, usize> = HashMap::new();

    for group in groups.values() {
        if group.is_empty() {
            continue;
        }

        let raw_mid = group.iter().map(|e| (e.sx + e.tx) / 2).sum::<usize>() / group.len();
        let bucket = raw_mid / LANE_BUCKET_WIDTH;
        let slot = lane_bucket_counts.entry(bucket).or_insert(0usize);
        let lane_x = (bucket * LANE_BUCKET_WIDTH + 2 + (*slot % LANE_BUCKET_SLOTS)).min(max_x);
        *slot = slot.saturating_add(1);

        let y_min = group.iter().map(|e| e.sy.min(e.ty)).min().unwrap_or(0);
        let y_max = group.iter().map(|e| e.sy.max(e.ty)).max().unwrap_or(0);

        let trunk_style = if group.iter().any(|e| e.highlighted) {
            Style::default()
                .fg(theme::YELLOW)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::SURFACE2)
        };

        draw_vline(&mut canvas, lane_x, y_min, y_max, trunk_style, glyphs);

        for edge in group {
            draw_hline(&mut canvas, edge.sx, lane_x, edge.sy, edge.style, glyphs);
            draw_hline(&mut canvas, lane_x, edge.tx, edge.ty, edge.style, glyphs);
            draw_arrow(
                &mut canvas,
                edge.tx,
                edge.ty,
                edge.left_to_right,
                edge.style,
                glyphs,
            );
        }
    }

    for (x, y, box_lines) in boxes_to_draw {
        write_box(&mut canvas, x, y, &box_lines);
    }

    canvas_to_lines(canvas)
}

// ---------------------------------------------------------------------------
// Helpers for building table box text
// ---------------------------------------------------------------------------

/// A styled line is a list of Spans (to carry color information).
#[derive(Clone)]
struct StyledLine {
    spans: Vec<Span<'static>>,
}

#[derive(Clone)]
struct CanvasCell {
    ch: char,
    style: Style,
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
struct CanvasRect {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

#[derive(Clone)]
struct PreparedEdge {
    from_idx: usize,
    to_idx: usize,
    sy: usize,
    ty: usize,
    sx: usize,
    tx: usize,
    left_to_right: bool,
    highlighted: bool,
    style: Style,
}

/// Height in lines of a rendered table box.
fn table_box_height(t: &TableSchema) -> usize {
    2 + t.columns.len() // top border + columns + bottom border (combined)
                        // Actually: top + N column rows + bottom = N + 2
}

use std::collections::HashSet;

/// Render a single table as a list of `StyledLine`s, each exactly BOX_WIDTH wide.
fn render_table_box(
    table: &TableSchema,
    is_selected: bool,
    is_related: bool,
    fk_from_cols: &HashSet<String>,
    fk_to_cols: &HashSet<String>,
    glyphs: GlyphSet,
) -> Vec<StyledLine> {
    let inner_width = (BOX_WIDTH as usize).saturating_sub(2); // subtract 2 for borders

    let border_style = if is_selected {
        Style::default()
            .fg(theme::BLUE)
            .add_modifier(Modifier::BOLD)
    } else if is_related {
        Style::default().fg(theme::YELLOW)
    } else {
        Style::default().fg(theme::OVERLAY0)
    };

    let title_style = if is_selected {
        Style::default()
            .fg(theme::BLUE)
            .add_modifier(Modifier::BOLD)
    } else if is_related {
        Style::default()
            .fg(theme::YELLOW)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(theme::TEXT)
            .add_modifier(Modifier::BOLD)
    };

    let mut lines: Vec<StyledLine> = Vec::new();

    // Top border: ┌─ schema.name ──...──┐
    let title = format!(" {}.{} ", table.schema, table.name);
    let title_len = title.len();
    let dashes_total = inner_width.saturating_sub(title_len);
    let dashes_right = dashes_total;
    let top = format!("┌{}{}┐", title, "─".repeat(dashes_right));
    let top_padded = pad_or_truncate(&top, BOX_WIDTH as usize);
    lines.push(StyledLine {
        spans: vec![
            Span::styled(glyphs.tl.to_string(), border_style),
            Span::styled(title, title_style),
            Span::styled(
                format!("{}{}", glyphs.h.to_string().repeat(dashes_right), glyphs.tr),
                border_style,
            ),
        ]
        .into_iter()
        .map(|sp| {
            // Ensure total width is correct — ratatui handles this
            sp
        })
        .collect(),
    });
    // Recalculate to ensure we emit exactly BOX_WIDTH characters
    // We rebuild the top line more carefully:
    let top_line = build_top_border(table, inner_width, border_style, title_style, glyphs);
    lines[0] = top_line;

    // Column rows
    for col in &table.columns {
        let indicator = if col.is_pk {
            Span::styled(
                "* ",
                Style::default()
                    .fg(theme::YELLOW)
                    .add_modifier(Modifier::BOLD),
            )
        } else if fk_from_cols.contains(&col.name) {
            Span::styled("→ ", Style::default().fg(theme::GREEN))
        } else if fk_to_cols.contains(&col.name) {
            Span::styled("← ", Style::default().fg(theme::MAUVE))
        } else {
            Span::raw("  ")
        };

        let nullable_marker = if col.is_nullable { " " } else { "!" };

        // inner content: "  col_name   type  " fits in inner_width
        // Layout: │ {ind}{col:<name_w}  {type:<type_w}{null} │
        // We have: 2 chars indicator, then name, gap, type, nullable, border
        // inner_width = 2 (ind) + name_w + 2 (gap) + type_w + 1 (null) = inner_width
        // name_w + type_w = inner_width - 5
        let avail = inner_width.saturating_sub(5); // ind(2) + gap(2) + null(1)
        let name_w = (avail * 2 / 3).max(8);
        let type_w = avail.saturating_sub(name_w);

        let name_truncated = truncate_str(&col.name, name_w);
        let type_truncated = truncate_str(&col.data_type, type_w);

        let name_padded = format!("{:<width$}", name_truncated, width = name_w);
        let type_padded = format!("{:<width$}", type_truncated, width = type_w);

        let content_style = if is_selected {
            Style::default().fg(theme::TEXT)
        } else {
            Style::default().fg(theme::OVERLAY2)
        };

        lines.push(StyledLine {
            spans: vec![
                Span::styled(glyphs.v.to_string(), border_style),
                indicator,
                Span::styled(name_padded, content_style),
                Span::raw("  "),
                Span::styled(type_padded, Style::default().fg(theme::OVERLAY0)),
                Span::raw(nullable_marker.to_string()),
                Span::styled(glyphs.v.to_string(), border_style),
            ],
        });
    }

    // Bottom border
    lines.push(StyledLine {
        spans: vec![Span::styled(
            format!(
                "{}{}{}",
                glyphs.bl,
                glyphs.h.to_string().repeat(inner_width),
                glyphs.br
            ),
            border_style,
        )],
    });

    // Suppress the unused variable warning
    let _ = top_padded;

    lines
}

/// Build the top border line with title inlined.
fn build_top_border(
    table: &TableSchema,
    inner_width: usize,
    border_style: Style,
    title_style: Style,
    glyphs: GlyphSet,
) -> StyledLine {
    let title = format!(" {}.{} ", table.schema, table.name);
    let title_len = title.chars().count();
    let right_dashes = inner_width.saturating_sub(title_len);

    StyledLine {
        spans: vec![
            Span::styled(glyphs.tl.to_string(), border_style),
            Span::styled(title, title_style),
            Span::styled(glyphs.h.to_string().repeat(right_dashes), border_style),
            Span::styled(glyphs.tr.to_string(), border_style),
        ],
    }
}

fn pad_or_truncate(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        s.chars().take(width).collect()
    } else {
        format!("{}{}", s, " ".repeat(width - len))
    }
}

fn truncate_str(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

fn write_box(canvas: &mut [Vec<CanvasCell>], x: usize, y: usize, box_lines: &[StyledLine]) {
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

fn endpoint_y(table: &TableSchema, rect: CanvasRect, col_name: &str) -> usize {
    let col_idx = table
        .columns
        .iter()
        .position(|c| c.name.eq_ignore_ascii_case(col_name));
    match col_idx {
        Some(i) => rect.y + 1 + i,
        None => rect.y + (rect.h / 2),
    }
}

fn draw_arrow(
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
    place_line_char(
        &mut canvas[y][x],
        if left_to_right { '>' } else { '<' },
        style,
        glyphs,
    );
}

fn draw_hline(
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
    let row_len = canvas[y].len();
    for x in start..=end {
        if x >= row_len {
            break;
        }
        place_line_char(&mut canvas[y][x], glyphs.h, style, glyphs);
    }
}

fn draw_vline(
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

fn place_line_char(cell: &mut CanvasCell, ch: char, style: Style, glyphs: GlyphSet) {
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

fn is_horizontal(ch: char) -> bool {
    matches!(
        ch,
        '-' | '+' | '─' | '┬' | '┴' | '├' | '┤' | '┼' | '┌' | '┐' | '└' | '┘'
    )
}

fn is_vertical(ch: char) -> bool {
    matches!(
        ch,
        '|' | '+' | '│' | '┬' | '┴' | '├' | '┤' | '┼' | '┌' | '┐' | '└' | '┘'
    )
}

fn canvas_to_lines(canvas: Vec<Vec<CanvasCell>>) -> Vec<Line<'static>> {
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

fn crop_line(line: Line<'static>, x_offset: usize, width: usize) -> Line<'static> {
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

fn line_width(line: &Line<'_>) -> usize {
    line.spans.iter().map(|sp| sp.content.chars().count()).sum()
}

// ---------------------------------------------------------------------------
// Help bar overlay at the bottom of the screen
// ---------------------------------------------------------------------------

fn draw_help_bar(frame: &mut Frame, full_area: Rect, focus_mode: bool) {
    if full_area.height < 2 {
        return;
    }
    let bar_area = Rect {
        x: full_area.x,
        y: full_area.y + full_area.height - 1,
        width: full_area.width,
        height: 1,
    };

    let help = Paragraph::new(Line::from(vec![
        Span::styled(" hjkl/arrows", Style::default().fg(theme::BLUE)),
        Span::raw(": scroll  "),
        Span::styled("wheel", Style::default().fg(theme::BLUE)),
        Span::raw(": y  "),
        Span::styled("shift+wheel", Style::default().fg(theme::BLUE)),
        Span::raw(": x  "),
        Span::styled("j/k/Tab", Style::default().fg(theme::BLUE)),
        Span::raw(": select table  "),
        Span::styled("f", Style::default().fg(theme::BLUE)),
        Span::raw(format!(
            ": {}  ",
            if focus_mode {
                "show all"
            } else {
                "focus selected"
            }
        )),
        Span::styled("u", Style::default().fg(theme::BLUE)),
        Span::raw(": ascii/unicode  "),
        Span::styled("Esc/q", Style::default().fg(theme::BLUE)),
        Span::raw(": close diagram "),
    ]))
    .style(Style::default().fg(theme::OVERLAY0));

    frame.render_widget(help, bar_area);
}
