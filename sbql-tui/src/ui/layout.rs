//! Layout: splits the terminal area into named regions.

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Named regions for the 4-panel layout.
pub struct Areas {
    /// Left column, top portion — connection list.
    /// Zero-height when the sidebar is hidden.
    pub connections: Rect,
    /// Left column, bottom portion — table list.
    /// Zero-height when the sidebar is hidden.
    pub tables: Rect,
    /// Right column — SQL editor (top) and results table (bottom).
    pub editor: Rect,
    pub results: Rect,
    /// Single-row status bar at the very bottom.
    pub status_bar: Rect,
}

/// Compute the panel layout from the full terminal `area`.
///
/// When `sidebar_hidden` is true the sidebar panels are collapsed to 0 width
/// and the editor/results columns expand to fill the full terminal width.
///
/// ```text
/// ┌──────────────┬────────────────────────────────────┐
/// │  Connections │  SQL Editor  (upper ~35%)           │
/// │  (25% × 40%) ├────────────────────────────────────┤
/// ├──────────────┤  Results Table (lower ~65%)         │
/// │  Tables      │                                     │
/// │  (25% × 60%) │                                     │
/// ├──────────────┴────────────────────────────────────┤
/// │  Status bar (1 row)                                │
/// └───────────────────────────────────────────────────┘
/// ```
pub fn compute(area: Rect, sidebar_hidden: bool) -> Areas {
    // Carve out the status bar at the bottom (1 row)
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(area);

    let main_area = outer[0];
    let status_bar = outer[1];

    // Horizontal split: sidebar (25%) | content (75%), or 0% | 100% when hidden
    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(if sidebar_hidden {
            [Constraint::Length(0), Constraint::Percentage(100)]
        } else {
            [Constraint::Percentage(25), Constraint::Percentage(75)]
        })
        .split(main_area);

    // Vertical split of left sidebar: 40% connections, 60% tables
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(h_chunks[0]);

    // Vertical split of right column: 35% editor, 65% results
    let v_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(h_chunks[1]);

    Areas {
        connections: left_chunks[0],
        tables: left_chunks[1],
        editor: v_chunks[0],
        results: v_chunks[1],
        status_bar,
    }
}
