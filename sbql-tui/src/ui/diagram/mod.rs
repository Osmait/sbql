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

mod canvas;

use canvas::*;

use crate::app::{DiagramGlyphMode, DiagramState};
use crate::ui::theme;

// ---------------------------------------------------------------------------
// Box layout constants
// ---------------------------------------------------------------------------

/// Fixed width of each table box (inner content width = BOX_WIDTH - 2 borders).
const BOX_WIDTH: u16 = 36;

/// Map a SQL data type string to a theme colour.
fn data_type_color(data_type: &str) -> ratatui::style::Color {
    let dt = data_type.to_ascii_lowercase();
    // Check temporal first (before numeric, since "timestamp"/"interval" contain "int")
    if dt.contains("date") || dt.contains("time") || dt.contains("interval") {
        theme::SKY
    // Structured types (before numeric, since "_int4" array types contain "int")
    } else if dt.contains("json")
        || dt.contains("xml")
        || dt.contains("array")
        || dt.contains("hstore")
        || dt.starts_with("_")
    {
        theme::MAUVE
    } else if dt.contains("bool") {
        theme::FLAMINGO
    } else if dt.contains("uuid") {
        theme::LAVENDER
    } else if dt.contains("bytea") || dt.contains("blob") || dt.contains("binary") {
        theme::MAROON
    } else if dt.contains("int")
        || dt.contains("float")
        || dt.contains("double")
        || dt.contains("serial")
        || dt.contains("decimal")
        || dt.contains("numeric")
        || dt.contains("real")
        || dt.starts_with("money")
    {
        theme::PEACH
    } else if dt.contains("char")
        || dt.contains("text")
        || dt.contains("citext")
        || dt.contains("name")
        || dt.contains("string")
    {
        theme::GREEN
    } else {
        theme::OVERLAY0
    }
}

// ---------------------------------------------------------------------------
// Public draw entry point
// ---------------------------------------------------------------------------

/// Where the diagram's two panes sit for a given screen.
fn panes(full: Rect) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(22), Constraint::Percentage(78)])
        .split(full)
}

/// Rebuild the canvas if stale, record the viewport, and pull scroll back into
/// range.
///
/// Everything that *changes* the diagram lives here. Rendering used to do this
/// mid-draw, which meant the scroll offset was only correct once a frame had
/// been painted — so anything reading it before then saw a stale value.
pub fn measure(state: &mut DiagramState, cache: &mut crate::ui::cache::RenderCache, full: Rect) {
    let area = panes(full)[1];
    let inner_h = area.height.saturating_sub(2) as usize;
    let inner_w = area.width.saturating_sub(2) as usize;

    state.last_viewport_w = inner_w as u16;
    state.last_viewport_h = inner_h as u16;

    if state.canvas_dirty || cache.diagram_canvas().is_none() {
        let build = build_canvas_lines(state, area.width);
        cache.set_diagram_canvas(build.lines);
        state.table_positions = build.table_positions;
        state.canvas_dirty = false;
    }

    let (canvas_height, canvas_width) = match cache.diagram_canvas() {
        Some(lines) => (lines.len(), lines.iter().map(line_width).max().unwrap_or(0)),
        None => (0, 0),
    };

    state.scroll_y = state
        .scroll_y
        .min(canvas_height.saturating_sub(inner_h) as u16);
    state.scroll_x = state
        .scroll_x
        .min(canvas_width.saturating_sub(inner_w) as u16);
}

/// Render the diagram. Reads state only — call [`measure`] first.
pub fn draw(frame: &mut Frame, state: &DiagramState, cache: &crate::ui::cache::RenderCache) {
    let full = frame.area();
    let split = panes(full);

    draw_sidebar(frame, state, split[0]);
    draw_canvas(frame, state, cache, split[1]);
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

    // Filter by search query if active
    let query_lower = state.search_query.to_ascii_lowercase();
    let filtered_indices: Vec<usize> = if state.search_active && !state.search_query.is_empty() {
        visible_indices
            .iter()
            .filter(|&&idx| {
                tables
                    .get(idx)
                    .map(|t| t.qualified().to_ascii_lowercase().contains(&query_lower))
                    .unwrap_or(false)
            })
            .copied()
            .collect()
    } else {
        visible_indices.clone()
    };

    let items: Vec<ListItem> = filtered_indices
        .iter()
        .filter_map(|&idx| tables.get(idx))
        .map(|t| ListItem::new(t.qualified()))
        .collect();
    let item_count = items.len();

    // Reserve bottom row for search input when active
    let (list_area, search_area) = if state.search_active && area.height > 3 {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(1)])
            .split(area);
        (split[0], Some(split[1]))
    } else {
        (area, None)
    };

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
    let selected_visible = filtered_indices
        .iter()
        .position(|&idx| idx == state.selected_table);
    list_state.select(if item_count == 0 {
        None
    } else {
        Some(selected_visible.unwrap_or(0))
    });

    frame.render_stateful_widget(list, list_area, &mut list_state);

    // Draw search input at bottom
    if let Some(sa) = search_area {
        let search_text = format!("/{}_", state.search_query);
        let search_line = Line::from(vec![
            Span::styled("/", Style::default().fg(theme::BLUE)),
            Span::styled(state.search_query.clone(), Style::default().fg(theme::TEXT)),
            Span::styled("_", Style::default().fg(theme::OVERLAY0)),
        ]);
        let _ = search_text; // suppress unused
        frame.render_widget(Paragraph::new(search_line), sa);
    }
}

// ---------------------------------------------------------------------------
// Right canvas: ASCII table boxes
// ---------------------------------------------------------------------------

fn draw_canvas(
    frame: &mut Frame,
    state: &DiagramState,
    cache: &crate::ui::cache::RenderCache,
    area: Rect,
) {
    let data = &state.data;
    let visible_indices = visible_table_indices(state);
    let visible_keys: std::collections::HashSet<String> = visible_indices
        .iter()
        .filter_map(|&i| data.tables.get(i))
        .map(|t| t.qualified())
        .collect();
    let visible_fk_count = visible_foreign_keys(state, &visible_keys).len();

    let inner_h = area.height.saturating_sub(2) as usize;
    let inner_w = area.width.saturating_sub(2) as usize;
    // Borrowed, not copied: the cached canvas is rebuilt only when the diagram
    // changes, but this runs every frame, so cloning every row of it — most of
    // them off-screen — was pure waste. Only the visible rows get cloned.
    let lines = cache.diagram_canvas().unwrap_or_default();
    let canvas_height = lines.len();

    // Position indicator
    let pct = if canvas_height > 0 {
        (state.scroll_y as usize * 100) / canvas_height.max(1)
    } else {
        0
    };

    // Apply scroll (vertical + horizontal)
    let scrolled: Vec<Line> = lines
        .iter()
        .skip(state.scroll_y as usize)
        .take(inner_h)
        .map(|line| crop_line(line.clone(), state.scroll_x as usize, inner_w))
        .collect();

    let paragraph = Paragraph::new(scrolled)
        .block(Block::default().borders(Borders::ALL).title(format!(
            " Diagram ({} tables, {} FKs) [x:{} y:{}] {}% ",
            visible_indices.len(),
            visible_fk_count,
            state.scroll_x,
            state.scroll_y,
            pct,
        )))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

// ---------------------------------------------------------------------------
// Canvas line builder
// ---------------------------------------------------------------------------

struct CanvasBuild {
    lines: Vec<Line<'static>>,
    /// Global table index → (x, y) position on canvas.
    table_positions: std::collections::HashMap<usize, (usize, usize)>,
}

/// Build the full virtual canvas as a vector of ratatui `Line`s.
fn build_canvas_lines(state: &DiagramState, _canvas_width: u16) -> CanvasBuild {
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
        return CanvasBuild {
            lines: vec![Line::from(Span::styled(
                "  No tables found in the current database.",
                Style::default().fg(theme::OVERLAY0),
            ))],
            table_positions: std::collections::HashMap::new(),
        };
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

    // Hierarchical directed layout with crossing minimization.
    const NODE_Y_GAP: usize = 2;
    const COMPONENT_GAP: usize = 4;

    let n_vis = visible_tables.len();
    let mut key_to_visible: HashMap<String, usize> = HashMap::new();
    for (vidx, t) in visible_tables.iter().enumerate() {
        key_to_visible.insert(t.qualified(), vidx);
    }

    // Build directed edges (from_table → to_table) and undirected adjacency for components
    let mut directed_edges: Vec<(usize, usize)> = Vec::new();
    let mut undirected_adj: Vec<Vec<usize>> = vec![Vec::new(); n_vis];
    for fk in &visible_fks {
        let from_key = format!("{}.{}", fk.from_schema, fk.from_table);
        let to_key = format!("{}.{}", fk.to_schema, fk.to_table);
        let (Some(&a), Some(&b)) = (key_to_visible.get(&from_key), key_to_visible.get(&to_key))
        else {
            continue;
        };
        if !directed_edges.contains(&(a, b)) {
            directed_edges.push((a, b));
        }
        if !undirected_adj[a].contains(&b) {
            undirected_adj[a].push(b);
        }
        if !undirected_adj[b].contains(&a) {
            undirected_adj[b].push(a);
        }
    }

    let selected_visible_idx = visible_indices
        .iter()
        .position(|&gidx| gidx == selected_idx);

    let mut positions: Vec<(usize, usize)> = vec![(0, 0); n_vis];
    let mut seen = vec![false; n_vis];
    let mut component_starts: Vec<usize> = (0..n_vis).collect();
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

        // Discover component via undirected DFS
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

        // Directed longest-path layer assignment:
        // All nodes start at layer 0, then for each directed edge from→to,
        // ensure layer[to] > layer[from].
        let comp_set: HashSet<usize> = component_nodes.iter().copied().collect();
        let mut layer: Vec<usize> = vec![0; n_vis];
        // Iterate multiple times to propagate (like Bellman-Ford)
        for _ in 0..component_nodes.len() {
            let mut changed = false;
            for &(a, b) in &directed_edges {
                if !comp_set.contains(&a) {
                    continue;
                }
                if layer[b] <= layer[a] {
                    layer[b] = layer[a] + 1;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        // Assign unconnected nodes via BFS from the selected or root node
        let root = if let Some(sel) = selected_visible_idx {
            if component_nodes.contains(&sel) {
                sel
            } else {
                *component_nodes
                    .iter()
                    .min_by_key(|&&n| (layer[n], visible_tables[n].qualified()))
                    .unwrap_or(&component_nodes[0])
            }
        } else {
            *component_nodes
                .iter()
                .min_by_key(|&&n| (layer[n], visible_tables[n].qualified()))
                .unwrap_or(&component_nodes[0])
        };

        // For nodes with no directed edges, use BFS layer from root
        let has_directed: HashSet<usize> = directed_edges
            .iter()
            .flat_map(|&(a, b)| [a, b])
            .filter(|n| comp_set.contains(n))
            .collect();
        if has_directed.len() < component_nodes.len() {
            let mut bfs_level: Vec<Option<usize>> = vec![None; n_vis];
            let mut q = std::collections::VecDeque::new();
            bfs_level[root] = Some(0);
            q.push_back(root);
            while let Some(n) = q.pop_front() {
                let base = bfs_level[n].unwrap_or(0);
                for &nb in &undirected_adj[n] {
                    if bfs_level[nb].is_none() && comp_set.contains(&nb) {
                        bfs_level[nb] = Some(base + 1);
                        q.push_back(nb);
                    }
                }
            }
            for &n in &component_nodes {
                if !has_directed.contains(&n) {
                    layer[n] = bfs_level[n].unwrap_or(0);
                }
            }
        }

        let max_layer = component_nodes.iter().map(|&n| layer[n]).max().unwrap_or(0);
        let mut layers: Vec<Vec<usize>> = vec![Vec::new(); max_layer + 1];
        for &n in &component_nodes {
            layers[layer[n]].push(n);
        }
        // Initial ordering: alphabetical
        for layer_nodes in &mut layers {
            layer_nodes.sort_by_key(|&n| visible_tables[n].qualified());
        }

        // Barycenter crossing minimization (4 passes)
        for _ in 0..4 {
            for li in 1..layers.len() {
                let prev = &layers[li - 1];
                let prev_pos: HashMap<usize, f64> = prev
                    .iter()
                    .enumerate()
                    .map(|(pos, &node)| (node, pos as f64))
                    .collect();

                let mut scored: Vec<(usize, f64)> = layers[li]
                    .iter()
                    .map(|&n| {
                        let neighbors: Vec<f64> = undirected_adj[n]
                            .iter()
                            .filter_map(|nb| prev_pos.get(nb).copied())
                            .collect();
                        let bc = if neighbors.is_empty() {
                            f64::MAX
                        } else {
                            neighbors.iter().sum::<f64>() / neighbors.len() as f64
                        };
                        (n, bc)
                    })
                    .collect();
                scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                layers[li] = scored.into_iter().map(|(n, _)| n).collect();
            }
        }

        // Adaptive gap: based on number of FKs between adjacent layers
        let row_stride = component_nodes
            .iter()
            .map(|&n| table_box_height(visible_tables[n]))
            .max()
            .unwrap_or(3)
            + NODE_Y_GAP;
        let max_rows = layers.iter().map(Vec::len).max().unwrap_or(1);

        let mut layer_x_offsets: Vec<usize> = Vec::new();
        let mut cum_x = 0usize;
        for li in 0..layers.len() {
            layer_x_offsets.push(cum_x);
            if li + 1 < layers.len() {
                // Count FKs between this layer and the next
                let this_set: HashSet<usize> = layers[li].iter().copied().collect();
                let next_set: HashSet<usize> = layers[li + 1].iter().copied().collect();
                let fk_count = directed_edges
                    .iter()
                    .filter(|&&(a, b)| {
                        (this_set.contains(&a) && next_set.contains(&b))
                            || (this_set.contains(&b) && next_set.contains(&a))
                    })
                    .count();
                let gap = 10 + (fk_count * 3).min(10); // range [10, 20]
                cum_x += BOX_WIDTH as usize + gap;
            }
        }

        for (li, layer_nodes) in layers.iter().enumerate() {
            let lx = layer_x_offsets[li];
            for (ly, &node) in layer_nodes.iter().enumerate() {
                positions[node] = (lx, comp_y + ly * row_stride);
            }
        }

        comp_y += max_rows * row_stride + COMPONENT_GAP;
    }

    let mut rects: HashMap<String, CanvasRect> = HashMap::new();
    let mut table_idx_by_key: HashMap<String, usize> = HashMap::new();
    let mut boxes_to_draw: Vec<(usize, usize, Vec<StyledLine>)> = Vec::new();
    let mut canvas_w = 1usize;
    let mut canvas_h = 1usize;
    let mut out_table_positions: HashMap<usize, (usize, usize)> = HashMap::new();

    for (vidx, t) in visible_tables.iter().enumerate() {
        let global_idx = visible_indices[vidx];
        let is_selected = global_idx == selected_idx;
        let is_related = related_tables.contains(&t.qualified());
        let empty_set = HashSet::new();
        let from_cols = fk_from.get(&t.qualified()).unwrap_or(&empty_set);
        let to_cols = fk_to.get(&t.qualified()).unwrap_or(&empty_set);
        let box_lines = render_table_box(
            t,
            is_selected,
            is_related,
            from_cols,
            to_cols,
            glyphs,
            state.glyph_mode,
        );
        let (x, y) = positions[vidx];

        canvas_w = canvas_w.max(x + BOX_WIDTH as usize + 3);
        canvas_h = canvas_h.max(y + box_lines.len() + 2);

        boxes_to_draw.push((x, y, box_lines.clone()));
        out_table_positions.insert(global_idx, (x, y));

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

    // Rotative colour palette for FK connectors (deterministic by table pair hash).
    const CONNECTOR_COLORS: [ratatui::style::Color; 6] = [
        theme::TEAL,
        theme::PINK,
        theme::PEACH,
        theme::SAPPHIRE,
        theme::FLAMINGO,
        theme::LAVENDER,
    ];

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

        // Deterministic colour based on the table pair
        let pair_hash = {
            let (lo, hi) = if from_idx <= to_idx {
                (from_idx, to_idx)
            } else {
                (to_idx, from_idx)
            };
            lo.wrapping_mul(131) ^ hi.wrapping_mul(97)
        };
        let base_color = CONNECTOR_COLORS[pair_hash % CONNECTOR_COLORS.len()];
        let style = if highlighted {
            Style::default().fg(base_color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(base_color)
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

    // Collect rect list for lane avoidance
    let rect_list: Vec<CanvasRect> = rects.values().copied().collect();

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
        let candidate_x = (bucket * LANE_BUCKET_WIDTH + 2 + (*slot % LANE_BUCKET_SLOTS)).min(max_x);
        *slot = slot.saturating_add(1);

        let y_min = group.iter().map(|e| e.sy.min(e.ty)).min().unwrap_or(0);
        let y_max = group.iter().map(|e| e.sy.max(e.ty)).max().unwrap_or(0);

        // Shift lane if it overlaps with any table box
        let lane_x = find_free_lane(candidate_x, &rect_list, y_min, y_max, max_x);

        // Trunk style: use colour of first highlighted edge, or first edge
        let trunk_style = if let Some(e) = group.iter().find(|e| e.highlighted) {
            e.style
        } else {
            group[0].style
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

            // Cardinality labels: N on source side, 1 on target side
            let label_style = Style::default().fg(theme::OVERLAY1);
            // Place "N" one row above source endpoint
            if edge.sy > 0 {
                let ny = edge.sy - 1;
                if ny < canvas.len() && edge.sx < canvas[ny].len() && canvas[ny][edge.sx].ch == ' '
                {
                    canvas[ny][edge.sx] = CanvasCell {
                        ch: 'N',
                        style: label_style,
                    };
                }
            }
            // Place "1" one row above target endpoint
            if edge.ty > 0 {
                let oy = edge.ty - 1;
                if oy < canvas.len() && edge.tx < canvas[oy].len() && canvas[oy][edge.tx].ch == ' '
                {
                    canvas[oy][edge.tx] = CanvasCell {
                        ch: '1',
                        style: label_style,
                    };
                }
            }
        }
    }

    for (x, y, box_lines) in boxes_to_draw {
        write_box(&mut canvas, x, y, &box_lines);
    }

    CanvasBuild {
        lines: canvas_to_lines(canvas),
        table_positions: out_table_positions,
    }
}

// ---------------------------------------------------------------------------
// Helpers for building table box text
// ---------------------------------------------------------------------------

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
/// top border + separator + N column rows + bottom border = N + 3
fn table_box_height(t: &TableSchema) -> usize {
    3 + t.columns.len()
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
    glyph_mode: DiagramGlyphMode,
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

    // Top border with title and column count: ┌ schema.table (N) ──...──┐
    let top_line = build_top_border(table, inner_width, border_style, title_style, glyphs);
    lines.push(top_line);

    // Separator line: ├──...──┤
    lines.push(StyledLine {
        spans: vec![Span::styled(
            format!(
                "{}{}{}",
                glyphs.ltee,
                glyphs.h.to_string().repeat(inner_width),
                glyphs.rtee
            ),
            border_style,
        )],
    });

    // Column rows
    for col in &table.columns {
        let indicator = if col.is_pk {
            let glyph = match glyph_mode {
                DiagramGlyphMode::Ascii => "* ",
                DiagramGlyphMode::Unicode => "\u{25c6} ", // ◆
            };
            Span::styled(
                glyph,
                Style::default()
                    .fg(theme::YELLOW)
                    .add_modifier(Modifier::BOLD),
            )
        } else if fk_from_cols.contains(&col.name) {
            let glyph = match glyph_mode {
                DiagramGlyphMode::Ascii => "->",
                DiagramGlyphMode::Unicode => "\u{2192} ", // →
            };
            Span::styled(glyph, Style::default().fg(theme::GREEN))
        } else if fk_to_cols.contains(&col.name) {
            let glyph = match glyph_mode {
                DiagramGlyphMode::Ascii => "<-",
                DiagramGlyphMode::Unicode => "\u{2190} ", // ←
            };
            Span::styled(glyph, Style::default().fg(theme::MAUVE))
        } else {
            Span::raw("  ")
        };

        let nullable_marker = if col.is_nullable { " " } else { "!" };

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

        let type_color = data_type_color(&col.data_type);

        lines.push(StyledLine {
            spans: vec![
                Span::styled(glyphs.v.to_string(), border_style),
                indicator,
                Span::styled(name_padded, content_style),
                Span::raw("  "),
                Span::styled(type_padded, Style::default().fg(type_color)),
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

    lines
}

/// Build the top border line with title and column count inlined.
fn build_top_border(
    table: &TableSchema,
    inner_width: usize,
    border_style: Style,
    title_style: Style,
    glyphs: GlyphSet,
) -> StyledLine {
    let title = format!(
        " {}.{} ({}) ",
        table.schema,
        table.name,
        table.columns.len()
    );
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

fn endpoint_y(table: &TableSchema, rect: CanvasRect, col_name: &str) -> usize {
    let col_idx = table
        .columns
        .iter()
        .position(|c| c.name.eq_ignore_ascii_case(col_name));
    match col_idx {
        // +2 = top border + separator line
        Some(i) => rect.y + 2 + i,
        None => rect.y + (rect.h / 2),
    }
}

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
        Span::styled(" hjkl", Style::default().fg(theme::BLUE)),
        Span::raw(": scroll  "),
        Span::styled("j/k/Tab", Style::default().fg(theme::BLUE)),
        Span::raw(": select  "),
        Span::styled("/", Style::default().fg(theme::BLUE)),
        Span::raw(": search  "),
        Span::styled("PgUp/Dn", Style::default().fg(theme::BLUE)),
        Span::raw(": fast scroll  "),
        Span::styled("f", Style::default().fg(theme::BLUE)),
        Span::raw(format!(
            ": {}  ",
            if focus_mode { "show all" } else { "focus" }
        )),
        Span::styled("u", Style::default().fg(theme::BLUE)),
        Span::raw(": glyph  "),
        Span::styled("Esc/q", Style::default().fg(theme::BLUE)),
        Span::raw(": close "),
    ]))
    .style(Style::default().fg(theme::OVERLAY0));

    frame.render_widget(help, bar_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};
    use sbql_core::{ColumnInfo, DiagramData, ForeignKey, TableSchema};

    /// Scroll clamping is a state change, so it belongs to `measure`. It used
    /// to happen mid-draw, which left the offset stale for anything that read
    /// it before the next frame was painted.
    #[test]
    fn measure_clamps_scroll_without_rendering() {
        let data = DiagramData {
            tables: vec![TableSchema {
                schema: "public".into(),
                name: "solo".into(),
                columns: vec![ColumnInfo {
                    name: "id".into(),
                    data_type: "integer".into(),
                    is_pk: true,
                    is_nullable: false,
                }],
            }],
            foreign_keys: vec![],
        };
        let mut state = DiagramState::new(data);
        state.scroll_x = 9_000;
        state.scroll_y = 9_000;

        let full = Rect {
            x: 0,
            y: 0,
            width: 120,
            height: 40,
        };
        let mut cache = crate::ui::cache::RenderCache::new();
        measure(&mut state, &mut cache, full);

        assert!(
            state.scroll_y < 9_000,
            "vertical scroll should be pulled back"
        );
        assert!(
            state.scroll_x < 9_000,
            "horizontal scroll should be pulled back"
        );
        assert!(cache.diagram_canvas().is_some(), "canvas built by measure");
        assert!(!state.canvas_dirty);
        assert_eq!(
            state.last_viewport_h, 38,
            "viewport recorded without a frame"
        );
    }

    #[test]
    fn test_diagram_rendering() {
        // Setup mock diagram data
        let data = DiagramData {
            tables: vec![
                TableSchema {
                    schema: "public".into(),
                    name: "users".into(),
                    columns: vec![
                        ColumnInfo {
                            name: "id".into(),
                            data_type: "integer".into(),
                            is_pk: true,
                            is_nullable: false,
                        },
                        ColumnInfo {
                            name: "name".into(),
                            data_type: "text".into(),
                            is_pk: false,
                            is_nullable: false,
                        },
                    ],
                },
                TableSchema {
                    schema: "public".into(),
                    name: "posts".into(),
                    columns: vec![
                        ColumnInfo {
                            name: "id".into(),
                            data_type: "integer".into(),
                            is_pk: true,
                            is_nullable: false,
                        },
                        ColumnInfo {
                            name: "user_id".into(),
                            data_type: "integer".into(),
                            is_pk: false,
                            is_nullable: false,
                        },
                    ],
                },
            ],
            foreign_keys: vec![ForeignKey {
                from_schema: "public".into(),
                from_table: "posts".into(),
                from_col: "user_id".into(),
                to_schema: "public".into(),
                to_table: "users".into(),
                to_col: "id".into(),
                constraint_name: "fk_user".into(),
            }],
        };

        let mut state = DiagramState::new(data);

        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        // Draw the UI
        let mut cache = crate::ui::cache::RenderCache::new();
        terminal
            .draw(|f| {
                // measure settles the canvas cache; draw is a pure read.
                measure(&mut state, &mut cache, f.area());
                draw(f, &state, &cache);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let mut content = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                content.push_str(buffer.cell((x, y)).unwrap().symbol());
            }
        }

        // Verify it rendered the table names
        assert!(content.contains("public.users"));
        assert!(content.contains("public.posts"));
        // Verify PK indicator
        assert!(content.contains("* id"));
        // Verify column
        assert!(content.contains("name"));
    }

    #[test]
    fn test_line_cropping() {
        let line = Line::from("hello world");
        let cropped = crop_line(line, 6, 5);
        // Asserted on the visible text rather than the span count: spans now
        // hold runs of same-styled characters instead of one character each.
        let text: String = cropped
            .spans
            .iter()
            .flat_map(|sp| sp.content.chars())
            .collect();
        assert_eq!(text, "world");
        assert_eq!(line_width(&cropped), 5);
    }

    #[test]
    fn test_data_type_color_numeric() {
        assert_eq!(data_type_color("integer"), theme::PEACH);
        assert_eq!(data_type_color("bigint"), theme::PEACH);
        assert_eq!(data_type_color("float8"), theme::PEACH);
        assert_eq!(data_type_color("serial"), theme::PEACH);
        assert_eq!(data_type_color("numeric(10,2)"), theme::PEACH);
    }

    #[test]
    fn test_data_type_color_text() {
        assert_eq!(data_type_color("varchar"), theme::GREEN);
        assert_eq!(data_type_color("text"), theme::GREEN);
        assert_eq!(data_type_color("character varying"), theme::GREEN);
    }

    #[test]
    fn test_data_type_color_boolean() {
        assert_eq!(data_type_color("boolean"), theme::FLAMINGO);
        assert_eq!(data_type_color("bool"), theme::FLAMINGO);
    }

    #[test]
    fn test_data_type_color_temporal() {
        assert_eq!(data_type_color("timestamp"), theme::SKY);
        assert_eq!(data_type_color("date"), theme::SKY);
        assert_eq!(data_type_color("time"), theme::SKY);
        assert_eq!(data_type_color("interval"), theme::SKY);
    }

    #[test]
    fn test_data_type_color_structured() {
        assert_eq!(data_type_color("jsonb"), theme::MAUVE);
        assert_eq!(data_type_color("json"), theme::MAUVE);
        assert_eq!(data_type_color("xml"), theme::MAUVE);
        assert_eq!(data_type_color("_int4"), theme::MAUVE); // array type
    }

    #[test]
    fn test_data_type_color_uuid() {
        assert_eq!(data_type_color("uuid"), theme::LAVENDER);
    }

    #[test]
    fn test_data_type_color_binary() {
        assert_eq!(data_type_color("bytea"), theme::MAROON);
    }

    #[test]
    fn test_data_type_color_default() {
        assert_eq!(data_type_color("oid"), theme::OVERLAY0);
        assert_eq!(data_type_color("void"), theme::OVERLAY0);
    }

    #[test]
    fn test_find_free_lane_no_overlap() {
        // No rects → lane stays put
        let rects = vec![];
        assert_eq!(find_free_lane(10, &rects, 0, 20, 100), 10);
    }

    #[test]
    fn test_find_free_lane_with_overlap() {
        let rects = vec![CanvasRect {
            x: 8,
            y: 0,
            w: 36,
            h: 10,
        }];
        // lane_x=10 is inside the rect (8..44), should shift right
        let result = find_free_lane(10, &rects, 0, 5, 100);
        assert!(result >= 44, "lane should be outside rect, got {}", result);
    }

    #[test]
    fn test_find_free_lane_no_y_overlap() {
        let rects = vec![CanvasRect {
            x: 8,
            y: 0,
            w: 36,
            h: 10,
        }];
        // Lane y range is below the rect → no overlap
        assert_eq!(find_free_lane(10, &rects, 15, 20, 100), 10);
    }

    #[test]
    fn test_table_box_height() {
        let t = TableSchema {
            schema: "public".into(),
            name: "test".into(),
            columns: vec![
                ColumnInfo {
                    name: "id".into(),
                    data_type: "integer".into(),
                    is_pk: true,
                    is_nullable: false,
                },
                ColumnInfo {
                    name: "name".into(),
                    data_type: "text".into(),
                    is_pk: false,
                    is_nullable: true,
                },
            ],
        };
        // 3 (header + separator + bottom) + 2 columns = 5
        assert_eq!(table_box_height(&t), 5);
    }

    #[test]
    fn test_build_canvas_stores_positions() {
        let data = DiagramData {
            tables: vec![
                TableSchema {
                    schema: "public".into(),
                    name: "a".into(),
                    columns: vec![ColumnInfo {
                        name: "id".into(),
                        data_type: "integer".into(),
                        is_pk: true,
                        is_nullable: false,
                    }],
                },
                TableSchema {
                    schema: "public".into(),
                    name: "b".into(),
                    columns: vec![ColumnInfo {
                        name: "id".into(),
                        data_type: "integer".into(),
                        is_pk: true,
                        is_nullable: false,
                    }],
                },
            ],
            foreign_keys: vec![],
        };
        let state = DiagramState::new(data);
        let build = build_canvas_lines(&state, 100);
        assert_eq!(build.table_positions.len(), 2);
        assert!(build.table_positions.contains_key(&0));
        assert!(build.table_positions.contains_key(&1));
    }

    #[test]
    fn test_diagram_rendering_with_column_count() {
        let data = DiagramData {
            tables: vec![TableSchema {
                schema: "public".into(),
                name: "users".into(),
                columns: vec![
                    ColumnInfo {
                        name: "id".into(),
                        data_type: "integer".into(),
                        is_pk: true,
                        is_nullable: false,
                    },
                    ColumnInfo {
                        name: "name".into(),
                        data_type: "text".into(),
                        is_pk: false,
                        is_nullable: false,
                    },
                ],
            }],
            foreign_keys: vec![],
        };

        let mut state = DiagramState::new(data);
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut cache = crate::ui::cache::RenderCache::new();
        terminal
            .draw(|f| {
                // measure settles the canvas cache; draw is a pure read.
                measure(&mut state, &mut cache, f.area());
                draw(f, &state, &cache);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let mut content = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                content.push_str(buffer.cell((x, y)).unwrap().symbol());
            }
        }

        // Column count should appear in the header
        assert!(
            content.contains("(2)"),
            "expected column count (2) in header"
        );
    }
}
