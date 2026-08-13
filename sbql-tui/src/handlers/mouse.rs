//! Turning mouse events into actions.
//!
//! Every click is resolved against the hit map the last frame recorded, so
//! this file never does arithmetic on panel coordinates: it asks what was
//! painted at a point and acts on the answer. That is what lets an overlay
//! take a click from the panel underneath it without either of them knowing
//! about the other.
//!
//! Two conventions the whole surface follows:
//! - A single click *selects*; a double click *acts*. A stray click can move a
//!   cursor, never open a connection or a table.
//! - The wheel scrolls whatever is under the pointer, not whatever has focus.
//!   Pointing at a list and turning the wheel has one obvious meaning.

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::action::{
    Action, CellEditAction, ConnectionsAction, DiagramAction, EditorAction, FilterAction,
    FormAction, NavAction, ResultsAction, TablesAction,
};
use crate::app::{AppState, FocusedPanel, NavMode};
use crate::ui::hit::{Side, Zone};

/// Map a mouse event to an [`Action`], the same contract as [`super::handle_key`].
///
/// Hit-testing needs the regions from the last draw, so this reads geometry as
/// well as state — but it stays a pure function of both.
pub fn handle(state: &AppState, mouse: MouseEvent) -> Action {
    let (col, row) = (mouse.column, mouse.row);

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => click(state, col, row),
        MouseEventKind::Drag(MouseButton::Left) => drag(state, col, row),
        MouseEventKind::Up(MouseButton::Left) => Action::Editor(EditorAction::DragEnd),
        MouseEventKind::ScrollDown => scroll(state, col, row, 1, mouse.modifiers),
        MouseEventKind::ScrollUp => scroll(state, col, row, -1, mouse.modifiers),
        _ => Action::Noop,
    }
}

// ---------------------------------------------------------------------------
// Clicks
// ---------------------------------------------------------------------------

fn click(state: &AppState, col: u16, row: u16) -> Action {
    let Some((rect, zone)) = state.layout.hits.hit(col, row) else {
        return Action::Noop;
    };
    let double = state.layout.is_double_click(col, row, state.tick);

    match zone {
        Zone::Panel(panel) => focus(panel),

        // -- Sidebar --
        Zone::ConnectionRow(i) => {
            let mut actions = focus_batch(FocusedPanel::Connections);
            actions.push(Action::Connections(ConnectionsAction::Select(i)));
            if double {
                actions.push(Action::Connections(ConnectionsAction::ConnectSelected));
            }
            Action::Batch(actions)
        }
        Zone::TableRow(i) => {
            let mut actions = focus_batch(FocusedPanel::Tables);
            actions.push(Action::Tables(TablesAction::Select(i)));
            if double {
                actions.push(Action::Tables(TablesAction::OpenSelected));
            }
            Action::Batch(actions)
        }

        // -- Results --
        Zone::ResultsCell { row: r, col: c } => {
            let mut actions = focus_batch(FocusedPanel::Results);
            actions.push(Action::Results(ResultsAction::SetRow(r)));
            actions.push(Action::Results(ResultsAction::SetCol(c)));
            if double {
                actions.push(Action::CellEdit(CellEditAction::Enter));
            }
            Action::Batch(actions)
        }
        Zone::ResultsHeader(c) => Action::Batch(vec![
            Action::Nav(NavAction::FocusPanel(FocusedPanel::Results)),
            Action::Nav(NavAction::SetNavMode(NavMode::Panel)),
            Action::Results(ResultsAction::SortColumn(c)),
        ]),
        Zone::ResultsColScroll(Side::Left) => Action::Results(ResultsAction::ColLeft),
        Zone::ResultsColScroll(Side::Right) => Action::Results(ResultsAction::ColRight),

        // -- Editor --
        Zone::EditorText => {
            let mut actions = focus_batch(FocusedPanel::Editor);
            if let Some((r, c)) = text_position(state, rect, col, row) {
                actions.push(Action::Editor(EditorAction::ClickAt { row: r, col: c }));
            }
            Action::Batch(actions)
        }

        // -- Overlays --
        Zone::FilterInput => Action::Noop,
        Zone::FilterSuggestion(i) => {
            let mut actions = vec![Action::Filter(FilterAction::SelectSuggestion(i))];
            if double {
                actions.push(Action::Filter(FilterAction::ApplySuggestion));
            }
            Action::Batch(actions)
        }
        Zone::FormField(i) => Action::Form(FormAction::FocusField(i)),
        Zone::FormSubmit => Action::Form(FormAction::Submit),
        Zone::FormCancel => Action::Form(FormAction::Close),
        Zone::CellEditInput => Action::Noop,
        Zone::CellEditStage => Action::CellEdit(CellEditAction::Stage),
        Zone::CellEditCancel => Action::CellEdit(CellEditAction::Cancel),
        // A click anywhere in the detail overlay closes it: it is a thing to
        // read, and the only thing to do with it is dismiss it.
        Zone::NoticeDetail => Action::CloseNoticeDetail,

        // -- Status bar --
        Zone::NoticeDetailHint => Action::ShowNoticeDetail,
        Zone::ConfirmDeleteYes => Action::Connections(ConnectionsAction::ConfirmDelete),
        Zone::ConfirmDeleteNo => Action::Connections(ConnectionsAction::CancelDelete),

        // -- Diagram --
        Zone::DiagramTable(i) => {
            let mut actions = vec![Action::Diagram(DiagramAction::SelectIndex(i))];
            if double {
                actions.push(Action::Diagram(DiagramAction::JumpToTable));
            }
            Action::Batch(actions)
        }
        Zone::DiagramCanvas => Action::Noop,
    }
}

// ---------------------------------------------------------------------------
// Drags
// ---------------------------------------------------------------------------

/// A drag continues whatever the button went down on.
///
/// Resolved against where the pointer *is* for the editor (the selection should
/// follow it out of the text area), but the diagram pans by how far the pointer
/// moved since the last event, so it needs the previous position rather than a
/// zone.
fn drag(state: &AppState, col: u16, row: u16) -> Action {
    if state.diagram.is_some() {
        let Some((last_col, last_row)) = state.layout.last_drag else {
            return Action::Noop;
        };
        // Dragging the canvas moves the content with the pointer, so the scroll
        // offset goes the other way — grabbing the paper, not the window.
        return Action::Diagram(DiagramAction::Scroll {
            dx: i32::from(last_col) as i16 - col as i16,
            dy: i32::from(last_row) as i16 - row as i16,
        });
    }

    // Only the editor supports drag-selection; the pointer may have left the
    // text area, so the anchor rect comes from where the drag began.
    let Some(rect) = state.layout.editor_text_rect else {
        return Action::Noop;
    };
    if !state.editor.dragging && !crate::ui::hit::contains(rect, col, row) {
        return Action::Noop;
    }
    match text_position(
        state,
        rect,
        col.clamp(rect.x, rect.right() - 1),
        row.clamp(rect.y, rect.bottom() - 1),
    ) {
        Some((r, c)) => Action::Editor(EditorAction::DragTo { row: r, col: c }),
        None => Action::Noop,
    }
}

// ---------------------------------------------------------------------------
// Wheel
// ---------------------------------------------------------------------------

/// Scroll whatever the pointer is over.
///
/// Following focus instead meant pointing at the results and turning the wheel
/// moved the connection list, which is the sort of thing that makes a TUI feel
/// like it is guessing.
fn scroll(state: &AppState, col: u16, row: u16, direction: i16, modifiers: KeyModifiers) -> Action {
    if state.diagram.is_some() {
        // Shift or Alt turns the wheel sideways, the usual terminal convention
        // for a mouse with no horizontal wheel.
        let sideways =
            modifiers.contains(KeyModifiers::SHIFT) || modifiers.contains(KeyModifiers::ALT);
        return Action::Diagram(if sideways {
            DiagramAction::Scroll {
                dx: direction * 4,
                dy: 0,
            }
        } else {
            DiagramAction::Scroll {
                dx: 0,
                dy: direction * 2,
            }
        });
    }

    // The pointer is usually over a row, but it is the panel that scrolls.
    let panel = state
        .layout
        .hits
        .hit_matching(col, row, |z| matches!(z, Zone::Panel(_)))
        .and_then(|(_, z)| match z {
            Zone::Panel(p) => Some(p),
            _ => None,
        });

    // A suggestion popup under the pointer scrolls itself, not the panel it
    // covers.
    if let Some((_, Zone::FilterSuggestion(_))) = state.layout.hits.hit(col, row) {
        return Action::Filter(if direction > 0 {
            FilterAction::SuggestionDown
        } else {
            FilterAction::SuggestionUp
        });
    }

    match panel {
        Some(FocusedPanel::Results) => Action::Results(if direction > 0 {
            ResultsAction::RowDown
        } else {
            ResultsAction::RowUp
        }),
        Some(FocusedPanel::Connections) => {
            if state.conn.is_empty() {
                return Action::Noop;
            }
            Action::Connections(ConnectionsAction::Select(step(
                state.conn.selected(),
                direction,
            )))
        }
        Some(FocusedPanel::Tables) => {
            if state.tables.tables.is_empty() {
                return Action::Noop;
            }
            Action::Tables(TablesAction::Select(step(
                state.tables.selected(),
                direction,
            )))
        }
        Some(FocusedPanel::Editor) => Action::Editor(EditorAction::Scroll(direction)),
        None => Action::Noop,
    }
}

/// Move an index one step, without going below zero.
fn step(current: usize, direction: i16) -> usize {
    if direction > 0 {
        current + 1
    } else {
        current.saturating_sub(1)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn focus(panel: FocusedPanel) -> Action {
    Action::Batch(focus_batch(panel))
}

fn focus_batch(panel: FocusedPanel) -> Vec<Action> {
    vec![
        Action::Nav(NavAction::FocusPanel(panel)),
        Action::Nav(NavAction::SetNavMode(NavMode::Panel)),
    ]
}

/// Turn a screen point inside the editor into a text position.
///
/// The editor scrolls, so the row is an offset from the first visible line,
/// and the column likewise. Clamping is left to tui-textarea, which knows the
/// real line lengths.
fn text_position(state: &AppState, rect: Rect, col: u16, row: u16) -> Option<(usize, usize)> {
    if !crate::ui::hit::contains(rect, col, row) {
        return None;
    }
    let line = state.editor.scroll_row + usize::from(row - rect.y);
    let column = state.editor.scroll_col + usize::from(col - rect.x);
    Some((line, column))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::hit::HitMap;

    fn state_with(zones: &[(Rect, Zone)]) -> AppState {
        let mut state = AppState::new(vec![]);
        let mut hits = HitMap::default();
        for (rect, zone) in zones {
            hits.register(*rect, *zone);
        }
        state.layout.hits = hits;
        state
    }

    fn left_click(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    fn wheel(col: u16, row: u16, down: bool) -> MouseEvent {
        MouseEvent {
            kind: if down {
                MouseEventKind::ScrollDown
            } else {
                MouseEventKind::ScrollUp
            },
            column: col,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    /// Flatten a batch so a test can ask "did this produce a connect?".
    fn contains_connect(action: &Action) -> bool {
        match action {
            Action::Connections(ConnectionsAction::ConnectSelected) => true,
            Action::Batch(inner) => inner.iter().any(contains_connect),
            _ => false,
        }
    }

    /// The rule the whole surface rests on: one click never acts.
    #[test]
    fn a_single_click_selects_but_does_not_connect() {
        let state = state_with(&[(Rect::new(0, 0, 20, 1), Zone::ConnectionRow(3))]);

        let action = handle(&state, left_click(5, 0));

        assert!(!contains_connect(&action), "{action:?}");
        assert!(
            matches!(&action, Action::Batch(a) if a.iter().any(|x| matches!(
                x, Action::Connections(ConnectionsAction::Select(3))
            ))),
            "{action:?}"
        );
    }

    #[test]
    fn a_second_click_in_the_same_spot_connects() {
        let mut state = state_with(&[(Rect::new(0, 0, 20, 1), Zone::ConnectionRow(3))]);
        state.tick = 10;
        state.layout.last_click = Some((5, 0, 9));

        let action = handle(&state, left_click(5, 0));

        assert!(contains_connect(&action), "{action:?}");
    }

    /// Same spot, too late: that is two separate clicks.
    #[test]
    fn a_slow_second_click_is_not_a_double_click() {
        let mut state = state_with(&[(Rect::new(0, 0, 20, 1), Zone::ConnectionRow(3))]);
        state.tick = 50;
        state.layout.last_click = Some((5, 0, 9));

        assert!(!contains_connect(&handle(&state, left_click(5, 0))));
    }

    /// A double-click that drifts a cell would act on the row next to the one
    /// the user pointed at.
    #[test]
    fn a_second_click_one_row_down_is_not_a_double_click() {
        let mut state = state_with(&[
            (Rect::new(0, 0, 20, 1), Zone::ConnectionRow(3)),
            (Rect::new(0, 1, 20, 1), Zone::ConnectionRow(4)),
        ]);
        state.tick = 10;
        state.layout.last_click = Some((5, 0, 9));

        assert!(!contains_connect(&handle(&state, left_click(5, 1))));
    }

    /// The overlay is registered last, so it takes the click.
    #[test]
    fn an_overlay_takes_the_click_from_the_panel_under_it() {
        let state = state_with(&[
            (Rect::new(0, 0, 80, 20), Zone::Panel(FocusedPanel::Results)),
            (Rect::new(10, 5, 20, 1), Zone::FormField(2)),
        ]);

        assert!(matches!(
            handle(&state, left_click(12, 5)),
            Action::Form(FormAction::FocusField(2))
        ));
    }

    #[test]
    fn clicking_a_header_sorts_that_column() {
        let state = state_with(&[(Rect::new(0, 0, 10, 1), Zone::ResultsHeader(2))]);

        assert!(
            matches!(&handle(&state, left_click(3, 0)), Action::Batch(a) if a.iter().any(|x| {
                matches!(x, Action::Results(ResultsAction::SortColumn(2)))
            })),
        );
    }

    /// The wheel follows the pointer. Focus is on the editor here, and the
    /// pointer is over the results.
    #[test]
    fn the_wheel_scrolls_what_is_under_the_pointer() {
        let mut state = state_with(&[
            (Rect::new(0, 0, 80, 10), Zone::Panel(FocusedPanel::Results)),
            (Rect::new(0, 3, 80, 1), Zone::ResultsCell { row: 3, col: 0 }),
        ]);
        state.focused = FocusedPanel::Editor;

        assert!(matches!(
            handle(&state, wheel(5, 3, true)),
            Action::Results(ResultsAction::RowDown)
        ));
        assert!(matches!(
            handle(&state, wheel(5, 3, false)),
            Action::Results(ResultsAction::RowUp)
        ));
    }

    #[test]
    fn the_wheel_over_nothing_does_nothing() {
        let state = state_with(&[]);
        assert!(matches!(handle(&state, wheel(5, 3, true)), Action::Noop));
    }

    /// A suggestion popup sits over the results panel; the wheel belongs to
    /// the popup while it is there.
    #[test]
    fn the_wheel_over_suggestions_moves_the_suggestion_cursor() {
        let state = state_with(&[
            (Rect::new(0, 0, 80, 10), Zone::Panel(FocusedPanel::Results)),
            (Rect::new(2, 4, 20, 1), Zone::FilterSuggestion(1)),
        ]);

        assert!(matches!(
            handle(&state, wheel(3, 4, true)),
            Action::Filter(FilterAction::SuggestionDown)
        ));
    }

    #[test]
    fn clicking_a_panel_body_only_focuses_it() {
        let state = state_with(&[(Rect::new(0, 0, 40, 10), Zone::Panel(FocusedPanel::Tables))]);

        assert!(
            matches!(&handle(&state, left_click(5, 5)), Action::Batch(a) if a.iter().any(|x| {
                matches!(x, Action::Nav(NavAction::FocusPanel(FocusedPanel::Tables)))
            })),
        );
    }

    #[test]
    fn clicking_outside_every_zone_does_nothing() {
        let state = state_with(&[(Rect::new(0, 0, 10, 10), Zone::Panel(FocusedPanel::Editor))]);
        assert!(matches!(handle(&state, left_click(50, 50)), Action::Noop));
    }

    /// Clicking the bar's affordance is the same as pressing Ctrl+E.
    #[test]
    fn clicking_the_details_hint_opens_the_detail() {
        let state = state_with(&[(Rect::new(60, 23, 19, 1), Zone::NoticeDetailHint)]);
        assert!(matches!(
            handle(&state, left_click(65, 23)),
            Action::ShowNoticeDetail
        ));
    }

    #[test]
    fn a_double_click_on_a_cell_opens_the_editor() {
        let mut state =
            state_with(&[(Rect::new(0, 5, 10, 1), Zone::ResultsCell { row: 2, col: 1 })]);
        state.tick = 3;
        state.layout.last_click = Some((4, 5, 2));

        assert!(
            matches!(&handle(&state, left_click(4, 5)), Action::Batch(a) if a.iter().any(|x| {
                matches!(x, Action::CellEdit(CellEditAction::Enter))
            })),
        );
    }

    // -- text_position --

    #[test]
    fn a_click_in_the_editor_maps_to_a_text_position() {
        let mut state = state_with(&[]);
        state.editor.scroll_row = 10;
        state.editor.scroll_col = 4;
        let rect = Rect::new(2, 3, 40, 10);

        assert_eq!(text_position(&state, rect, 2, 3), Some((10, 4)));
        assert_eq!(text_position(&state, rect, 7, 5), Some((12, 9)));
        assert_eq!(text_position(&state, rect, 100, 5), None);
    }
}
