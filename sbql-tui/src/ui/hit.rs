//! What sits where on screen, so a click can find it.
//!
//! Hit-testing used to be four rectangles — the four panels — which is why
//! everything drawn on top of them was dead to the mouse: the connection form,
//! the filter bar and its suggestions, the cell-edit popup, the notice detail.
//! A click landing on a form field was answered by the panel underneath it.
//!
//! Instead every draw records the regions it painted, in the order it painted
//! them. That ordering is the whole trick: an overlay is drawn last, so it is
//! registered last, so it wins the click — the same rule the screen itself
//! follows. No widget has to know what might be covering it.

use ratatui::layout::Rect;

use crate::app::FocusedPanel;

/// Which way a horizontal affordance points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Left,
    Right,
}

/// Something clickable, named by what it *is* rather than where it is.
///
/// The mouse handler matches on this, so adding a clickable thing is a variant
/// plus a `register` call at the point that draws it — never arithmetic on
/// panel coordinates performed somewhere far away from the drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Zone {
    /// The body of a panel. Clicking focuses it; the more specific zones
    /// registered on top say what else the click means.
    Panel(FocusedPanel),

    // -- Sidebar lists --
    /// Row `n` of the connections list (saved and discovered, as drawn).
    ConnectionRow(usize),
    /// Row `n` of the tables list.
    TableRow(usize),

    // -- Results grid --
    /// A cell, in data coordinates rather than screen ones.
    ResultsCell {
        row: usize,
        col: usize,
    },
    /// A column header. Clicking cycles that column's sort.
    ResultsHeader(usize),
    /// The `◀` / `▶` markers that appear when columns overflow.
    ResultsColScroll(Side),

    // -- Editor --
    /// The editable text. The handler turns the point into a text position
    /// using the rect this was registered with.
    EditorText,

    // -- Overlays --
    /// The filter input line.
    FilterInput,
    /// Row `n` of the filter suggestion popup.
    FilterSuggestion(usize),
    /// Field `n` of the connection form (row 0 is the backend picker).
    FormField(usize),
    /// The connection form's confirm/cancel affordances.
    FormSubmit,
    FormCancel,
    /// The cell-edit popup's text area.
    CellEditInput,
    /// The cell-edit popup's stage/cancel affordances.
    CellEditStage,
    CellEditCancel,
    /// The open notice-detail overlay. Clicking anywhere in it closes it.
    NoticeDetail,
    /// Row `n` of the theme picker.
    ThemeRow(usize),

    // -- Status bar --
    /// The bar's "(Ctrl+E: details)" affordance.
    NoticeDetailHint,
    /// The confirmation bar's yes/no.
    ConfirmDeleteYes,
    ConfirmDeleteNo,

    // -- Diagram --
    /// Row `n` of the diagram's table sidebar.
    DiagramTable(usize),
    /// The diagram canvas. Dragging pans it.
    DiagramCanvas,
}

/// The regions of the last frame, in the order they were painted.
#[derive(Debug, Default, Clone)]
pub(crate) struct HitMap {
    zones: Vec<(Rect, Zone)>,
}

impl HitMap {
    /// Forget the previous frame. Called once at the top of every draw, so a
    /// zone can never outlive the thing that drew it — a stale suggestion row
    /// would otherwise keep swallowing clicks after the popup closed.
    pub(crate) fn clear(&mut self) {
        self.zones.clear();
    }

    /// Record a region. Later calls win, so draw order is click order.
    pub(crate) fn register(&mut self, rect: Rect, zone: Zone) {
        // A zero-sized rect can never be hit and only makes the scan longer;
        // widgets legitimately produce them when a panel is collapsed.
        if rect.width > 0 && rect.height > 0 {
            self.zones.push((rect, zone));
        }
    }

    /// The topmost zone at a point, with the rect it was registered with.
    ///
    /// The rect comes back because the caller usually needs it: turning a click
    /// into a text position or a scroll offset is arithmetic against the region
    /// that was actually drawn.
    pub(crate) fn hit(&self, col: u16, row: u16) -> Option<(Rect, Zone)> {
        self.zones
            .iter()
            .rev()
            .find(|(rect, _)| contains(*rect, col, row))
            .copied()
    }

    /// The topmost zone at a point that satisfies `want`.
    ///
    /// For scrolling, where the pointer is usually over a row rather than the
    /// panel, but the panel is what scrolls.
    pub(crate) fn hit_matching(
        &self,
        col: u16,
        row: u16,
        want: impl Fn(Zone) -> bool,
    ) -> Option<(Rect, Zone)> {
        self.zones
            .iter()
            .rev()
            .find(|(rect, zone)| contains(*rect, col, row) && want(*zone))
            .copied()
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.zones.len()
    }
}

/// Whether a point falls inside a rect. Right and bottom edges are exclusive,
/// as ratatui's rects are.
pub(crate) fn contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_outside_everything_hits_nothing() {
        let mut hits = HitMap::default();
        hits.register(Rect::new(0, 0, 10, 10), Zone::EditorText);
        assert_eq!(hits.hit(50, 50), None);
    }

    /// The rule the whole module exists for: the thing drawn last is the thing
    /// the user is looking at, so it takes the click.
    #[test]
    fn the_last_registered_zone_wins() {
        let mut hits = HitMap::default();
        hits.register(Rect::new(0, 0, 40, 20), Zone::Panel(FocusedPanel::Results));
        hits.register(Rect::new(5, 5, 10, 3), Zone::FormField(2));

        let (_, zone) = hits.hit(6, 6).expect("a hit");
        assert_eq!(zone, Zone::FormField(2));

        // Outside the overlay, the panel underneath still answers.
        let (_, zone) = hits.hit(30, 15).expect("a hit");
        assert_eq!(zone, Zone::Panel(FocusedPanel::Results));
    }

    #[test]
    fn the_rect_comes_back_with_the_zone() {
        let mut hits = HitMap::default();
        let rect = Rect::new(3, 4, 20, 6);
        hits.register(rect, Zone::EditorText);

        assert_eq!(hits.hit(5, 5), Some((rect, Zone::EditorText)));
    }

    /// Scrolling asks "which panel is under the pointer", and the pointer is
    /// almost always over a row rather than the panel body.
    #[test]
    fn a_filtered_hit_looks_past_the_topmost_zone() {
        let mut hits = HitMap::default();
        hits.register(Rect::new(0, 0, 40, 20), Zone::Panel(FocusedPanel::Tables));
        hits.register(Rect::new(1, 3, 38, 1), Zone::TableRow(2));

        assert_eq!(hits.hit(5, 3).map(|(_, z)| z), Some(Zone::TableRow(2)));
        assert_eq!(
            hits.hit_matching(5, 3, |z| matches!(z, Zone::Panel(_)))
                .map(|(_, z)| z),
            Some(Zone::Panel(FocusedPanel::Tables))
        );
    }

    /// A popup that closed must not keep answering clicks where it used to be.
    #[test]
    fn clearing_forgets_the_previous_frame() {
        let mut hits = HitMap::default();
        hits.register(Rect::new(0, 0, 10, 10), Zone::FilterSuggestion(0));
        hits.clear();
        assert_eq!(hits.hit(5, 5), None);
    }

    #[test]
    fn an_empty_rect_is_not_registered() {
        let mut hits = HitMap::default();
        hits.register(Rect::new(5, 5, 0, 3), Zone::FormSubmit);
        hits.register(Rect::new(5, 5, 3, 0), Zone::FormCancel);
        assert_eq!(hits.len(), 0);
    }
}
