//! The full-screen schema diagram.

use super::*;

/// The full-screen schema diagram.
pub(super) fn apply(
    action: DiagramAction,
    state: &mut AppState,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    match action {
        // -- Diagram --
        DiagramAction::Open => {
            if state.conn.active_id.is_some() {
                state.diagram_requested = true;
                let _ = cmd_tx.send(CoreCommand::LoadDiagram);
            } else {
                state.error_msg =
                    Some("Connect to a database first (Enter on a connection).".into());
            }
        }

        DiagramAction::Close => {
            state.diagram = None;
        }

        DiagramAction::Scroll { dx, dy } => {
            if let Some(ref mut diag) = state.diagram {
                if dx > 0 {
                    diag.scroll_x = diag.scroll_x.saturating_add(dx as u16);
                } else {
                    diag.scroll_x = diag.scroll_x.saturating_sub((-dx) as u16);
                }
                if dy > 0 {
                    diag.scroll_y = diag.scroll_y.saturating_add(dy as u16);
                } else {
                    diag.scroll_y = diag.scroll_y.saturating_sub((-dy) as u16);
                }
            }
        }

        DiagramAction::SelectNext => {
            if let Some(ref mut diag) = state.diagram {
                let visible = diagram_visible_table_indices(diag);
                if !visible.is_empty() {
                    let pos = visible
                        .iter()
                        .position(|&i| i == diag.selected_table)
                        .unwrap_or(0);
                    let next = (pos + 1).min(visible.len() - 1);
                    diag.selected_table = visible[next];
                    diag.canvas_dirty = true;
                    diagram_keep_in_view(diag);
                }
            }
        }

        DiagramAction::SelectPrev => {
            if let Some(ref mut diag) = state.diagram {
                let visible = diagram_visible_table_indices(diag);
                if !visible.is_empty() {
                    let pos = visible
                        .iter()
                        .position(|&i| i == diag.selected_table)
                        .unwrap_or(0);
                    diag.selected_table = visible[pos.saturating_sub(1)];
                    diag.canvas_dirty = true;
                    diagram_keep_in_view(diag);
                }
            }
        }

        DiagramAction::SelectFirst => {
            if let Some(ref mut diag) = state.diagram {
                let visible = diagram_visible_table_indices(diag);
                if !visible.is_empty() {
                    diag.selected_table = visible[0];
                }
                diag.scroll_y = 0;
                diag.canvas_dirty = true;
            }
        }

        DiagramAction::SelectLast => {
            if let Some(ref mut diag) = state.diagram {
                let visible = diagram_visible_table_indices(diag);
                if !visible.is_empty() {
                    diag.selected_table = visible[visible.len() - 1];
                }
                diag.canvas_dirty = true;
            }
        }

        DiagramAction::ToggleFocus => {
            if let Some(ref mut diag) = state.diagram {
                diag.focus_mode = !diag.focus_mode;
                diag.canvas_dirty = true;
                if diag.focus_mode {
                    let vis = diagram_visible_table_indices(diag);
                    if !vis.is_empty() && !vis.contains(&diag.selected_table) {
                        diag.selected_table = vis[0];
                    }
                }
            }
        }

        DiagramAction::ToggleGlyph => {
            if let Some(ref mut diag) = state.diagram {
                diag.glyph_mode = match diag.glyph_mode {
                    DiagramGlyphMode::Ascii => DiagramGlyphMode::Unicode,
                    DiagramGlyphMode::Unicode => DiagramGlyphMode::Ascii,
                };
                diag.canvas_dirty = true;
            }
        }

        DiagramAction::JumpToTable => {
            if let Some(ref mut diag) = state.diagram {
                if let Some(&(tx, ty)) = diag.table_positions.get(&diag.selected_table) {
                    let vw = diag.last_viewport_w as usize;
                    let vh = diag.last_viewport_h as usize;
                    diag.scroll_x = (tx.saturating_sub(vw / 2)) as u16;
                    diag.scroll_y = (ty.saturating_sub(vh / 2)) as u16;
                }
            }
        }

        DiagramAction::SearchOpen => {
            if let Some(ref mut diag) = state.diagram {
                diag.search_active = true;
                diag.search_query.clear();
            }
        }

        DiagramAction::SearchClose => {
            if let Some(ref mut diag) = state.diagram {
                diag.search_active = false;
                diag.search_query.clear();
            }
        }

        DiagramAction::SearchInput(c) => {
            if let Some(ref mut diag) = state.diagram {
                diag.search_query.push(c);
                // Auto-select first matching table
                let query = diag.search_query.to_ascii_lowercase();
                let visible = diagram_visible_table_indices(diag);
                if let Some(&idx) = visible.iter().find(|&&i| {
                    diag.data
                        .tables
                        .get(i)
                        .map(|t| t.qualified().to_ascii_lowercase().contains(&query))
                        .unwrap_or(false)
                }) {
                    diag.selected_table = idx;
                    diag.canvas_dirty = true;
                }
            }
        }

        DiagramAction::SearchBackspace => {
            if let Some(ref mut diag) = state.diagram {
                diag.search_query.pop();
                if !diag.search_query.is_empty() {
                    let query = diag.search_query.to_ascii_lowercase();
                    let visible = diagram_visible_table_indices(diag);
                    if let Some(&idx) = visible.iter().find(|&&i| {
                        diag.data
                            .tables
                            .get(i)
                            .map(|t| t.qualified().to_ascii_lowercase().contains(&query))
                            .unwrap_or(false)
                    }) {
                        diag.selected_table = idx;
                        diag.canvas_dirty = true;
                    }
                }
            }
        }

        DiagramAction::SearchConfirm => {
            if let Some(ref mut diag) = state.diagram {
                diag.search_active = false;
                // Jump to the selected table
                if let Some(&(tx, ty)) = diag.table_positions.get(&diag.selected_table) {
                    let vw = diag.last_viewport_w as usize;
                    let vh = diag.last_viewport_h as usize;
                    diag.scroll_x = (tx.saturating_sub(vw / 2)) as u16;
                    diag.scroll_y = (ty.saturating_sub(vh / 2)) as u16;
                }
                diag.search_query.clear();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers used by apply()
// ---------------------------------------------------------------------------

/// Scroll the diagram viewport so the selected table stays visible (without centering).
pub(super) fn diagram_keep_in_view(diag: &mut crate::app::DiagramState) {
    if let Some(&(tx, ty)) = diag.table_positions.get(&diag.selected_table) {
        let vw = diag.last_viewport_w as usize;
        let vh = diag.last_viewport_h as usize;
        let sx = diag.scroll_x as usize;
        let sy = diag.scroll_y as usize;
        // Horizontal keep-in-view
        if tx < sx {
            diag.scroll_x = tx as u16;
        } else if tx + 36 > sx + vw {
            diag.scroll_x = (tx + 36).saturating_sub(vw) as u16;
        }
        // Vertical keep-in-view
        if ty < sy {
            diag.scroll_y = ty as u16;
        } else if ty + 4 > sy + vh {
            diag.scroll_y = (ty + 4).saturating_sub(vh) as u16;
        }
    }
}

fn diagram_visible_table_indices(diag: &crate::app::DiagramState) -> Vec<usize> {
    let tables = &diag.data.tables;
    if tables.is_empty() {
        return Vec::new();
    }
    if !diag.focus_mode {
        return (0..tables.len()).collect();
    }

    let selected = diag.selected_table.min(tables.len().saturating_sub(1));
    let selected_key = tables[selected].qualified();
    let mut keys = std::collections::HashSet::new();
    keys.insert(selected_key.clone());

    for fk in &diag.data.foreign_keys {
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

// Make parse_filter_input available for testing.
