use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use tokio::sync::mpsc;

use crate::action::{self, Action};
use crate::app::{AppState, FocusedPanel, NavMode};
use sbql_core::CoreCommand;

/// Mouse events are applied directly (not via Action) because they involve
/// complex coordinate calculations that depend on mutable state.
/// This keeps the mouse handler pragmatic while keyboard handlers use the Action pattern.
pub fn handle(
    state: &mut AppState,
    mouse: MouseEvent,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    let act = map_mouse(state, mouse);
    action::apply(act, state, cmd_tx);
}

fn map_mouse(state: &AppState, mouse: MouseEvent) -> Action {
    if state.diagram.is_some() {
        return match mouse.kind {
            MouseEventKind::ScrollDown => {
                if mouse.modifiers.contains(KeyModifiers::SHIFT)
                    || mouse.modifiers.contains(KeyModifiers::ALT)
                {
                    Action::DiagramScroll { dx: 4, dy: 0 }
                } else {
                    Action::DiagramScroll { dx: 0, dy: 2 }
                }
            }
            MouseEventKind::ScrollUp => {
                if mouse.modifiers.contains(KeyModifiers::SHIFT)
                    || mouse.modifiers.contains(KeyModifiers::ALT)
                {
                    Action::DiagramScroll { dx: -4, dy: 0 }
                } else {
                    Action::DiagramScroll { dx: 0, dy: -2 }
                }
            }
            _ => Action::Noop,
        };
    }

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => map_mouse_click(state, mouse.column, mouse.row),
        MouseEventKind::ScrollDown => match state.focused {
            FocusedPanel::Results => Action::MoveRowDown,
            FocusedPanel::Connections => {
                if !state.conn.connections.is_empty() {
                    let next = (state.conn.selected + 1).min(state.conn.connections.len() - 1);
                    Action::SelectConnection(next)
                } else {
                    Action::Noop
                }
            }
            FocusedPanel::Tables => {
                if !state.tables.tables.is_empty() {
                    let next = (state.tables.selected + 1).min(state.tables.tables.len() - 1);
                    Action::SelectTable(next)
                } else {
                    Action::Noop
                }
            }
            _ => Action::Noop,
        },
        MouseEventKind::ScrollUp => match state.focused {
            FocusedPanel::Results => Action::MoveRowUp,
            FocusedPanel::Connections => {
                Action::SelectConnection(state.conn.selected.saturating_sub(1))
            }
            FocusedPanel::Tables => Action::SelectTable(state.tables.selected.saturating_sub(1)),
            _ => Action::Noop,
        },
        _ => Action::Noop,
    }
}

fn map_mouse_click(state: &AppState, col: u16, row: u16) -> Action {
    if let Some(la) = state.layout.last_areas {
        if rect_contains(la.table_list, col, row) {
            let mut actions = vec![
                Action::FocusPanel(FocusedPanel::Tables),
                Action::SetNavMode(NavMode::Panel),
            ];
            if row > la.table_list.y {
                let clicked = (row - la.table_list.y).saturating_sub(1) as usize;
                let new_idx = clicked.min(state.tables.tables.len().saturating_sub(1));
                actions.push(Action::SelectTable(new_idx));
            }
            return Action::Batch(actions);
        }
        if rect_contains(la.conn_list, col, row) {
            let mut actions = vec![
                Action::FocusPanel(FocusedPanel::Connections),
                Action::SetNavMode(NavMode::Panel),
            ];
            if row > la.conn_list.y && !state.conn.connections.is_empty() {
                let clicked = (row - la.conn_list.y).saturating_sub(1) as usize;
                let new_idx = clicked.min(state.conn.connections.len() - 1);
                actions.push(Action::SelectConnection(new_idx));
            }
            return Action::Batch(actions);
        }
        if rect_contains(la.editor, col, row) {
            return Action::Batch(vec![
                Action::FocusPanel(FocusedPanel::Editor),
                Action::SetNavMode(NavMode::Panel),
            ]);
        }
        if rect_contains(la.results, col, row) {
            let mut actions = vec![
                Action::FocusPanel(FocusedPanel::Results),
                Action::SetNavMode(NavMode::Panel),
            ];
            let header_offset = 2u16;
            if row >= la.results.y + header_offset {
                let clicked_row_vis = (row - la.results.y - header_offset) as usize;
                let new_row = state.results.scroll + clicked_row_vis;
                if new_row < state.results.data.rows.len() {
                    actions.push(Action::SetResultsRow(new_row));
                }
            }
            if !state.layout.last_col_widths.is_empty() && col > la.results.x {
                let inner_x = (col - la.results.x).saturating_sub(1) as usize;
                let col_scroll = state.results.col_scroll;
                let mut acc = 0usize;
                let mut clicked_col = col_scroll;
                for (ci, &w) in state
                    .layout
                    .last_col_widths
                    .iter()
                    .enumerate()
                    .skip(col_scroll)
                {
                    let next_acc = acc + w as usize + 1;
                    if inner_x < next_acc {
                        clicked_col = ci;
                        break;
                    }
                    acc = next_acc;
                    clicked_col = ci + 1;
                }
                let max_col = state.results.data.columns.len().saturating_sub(1);
                actions.push(Action::SetResultsCol(clicked_col.min(max_col)));
            }
            return Action::Batch(actions);
        }
    } else {
        // Fallback heuristic
        let term_width = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);
        let conn_width = term_width / 4;
        if col < conn_width {
            return Action::Batch(vec![
                Action::FocusPanel(FocusedPanel::Connections),
                Action::SetNavMode(NavMode::Panel),
            ]);
        }
        let term_height = crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24);
        let editor_height = term_height * 35 / 100;
        if row < editor_height {
            return Action::Batch(vec![
                Action::FocusPanel(FocusedPanel::Editor),
                Action::SetNavMode(NavMode::Panel),
            ]);
        }
        return Action::Batch(vec![
            Action::FocusPanel(FocusedPanel::Results),
            Action::SetNavMode(NavMode::Panel),
        ]);
    }
    Action::Noop
}

pub(crate) fn rect_contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_contains_inside() {
        let r = Rect::new(10, 20, 30, 10);
        assert!(rect_contains(r, 15, 25));
    }

    #[test]
    fn rect_contains_top_left_corner() {
        let r = Rect::new(10, 20, 30, 10);
        assert!(rect_contains(r, 10, 20));
    }

    #[test]
    fn rect_contains_outside_right() {
        let r = Rect::new(10, 20, 30, 10);
        assert!(!rect_contains(r, 40, 25)); // x=40 is at 10+30, exclusive
    }

    #[test]
    fn rect_contains_outside_bottom() {
        let r = Rect::new(10, 20, 30, 10);
        assert!(!rect_contains(r, 15, 30)); // y=30 is at 20+10, exclusive
    }

    #[test]
    fn rect_contains_outside_left() {
        let r = Rect::new(10, 20, 30, 10);
        assert!(!rect_contains(r, 9, 25));
    }

    #[test]
    fn rect_contains_outside_top() {
        let r = Rect::new(10, 20, 30, 10);
        assert!(!rect_contains(r, 15, 19));
    }

    #[test]
    fn rect_contains_bottom_right_edge_exclusive() {
        let r = Rect::new(0, 0, 10, 10);
        assert!(rect_contains(r, 9, 9));
        assert!(!rect_contains(r, 10, 10));
    }

    #[test]
    fn rect_contains_zero_size() {
        let r = Rect::new(5, 5, 0, 0);
        assert!(!rect_contains(r, 5, 5));
    }
}
