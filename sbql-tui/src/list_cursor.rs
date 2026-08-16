//! One selection cursor for every list in the app.
//!
//! Selection used to be hand-rolled at each call site, which produced six
//! subtly different answers to the same question: what happens at the end of a
//! list. The completion popup wrapped, the diagram clamped, the filter
//! suggestions clamped, connections clamped — none of it deliberate.
//!
//! [`ListCursor`] makes that choice explicit at the call site via [`Overflow`],
//! and keeps the bounds checking in one tested place.
//!
//! The cursor does not own the list. Length is passed in on every call, so it
//! can never disagree with the data it is pointing at.

/// What happens when moving past either end of a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Overflow {
    /// Stop at the end. Right for browsing lists, where overshooting and
    /// silently landing back at the top is disorienting.
    Clamp,
    /// Continue from the other end. Right for short popups, where cycling is
    /// faster than reversing.
    Wrap,
}

/// A bounds-checked index into a list held somewhere else.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ListCursor {
    index: usize,
}

impl ListCursor {
    pub(crate) fn new() -> Self {
        Self { index: 0 }
    }

    /// The selected index. Callers should still use `.get(i)` on their list —
    /// the cursor is only valid for the length it was last moved against.
    pub(crate) fn index(&self) -> usize {
        self.index
    }

    /// Move to a specific index, clamped into the list.
    pub(crate) fn select(&mut self, index: usize, len: usize) {
        self.index = if len == 0 { 0 } else { index.min(len - 1) };
    }

    pub(crate) fn next(&mut self, len: usize, overflow: Overflow) {
        if len == 0 {
            self.index = 0;
            return;
        }
        self.index = match overflow {
            Overflow::Wrap => (self.index + 1) % len,
            Overflow::Clamp => (self.index + 1).min(len - 1),
        };
    }

    pub(crate) fn prev(&mut self, len: usize, overflow: Overflow) {
        if len == 0 {
            self.index = 0;
            return;
        }
        self.index = match overflow {
            Overflow::Wrap if self.index == 0 => len - 1,
            Overflow::Wrap => self.index - 1,
            Overflow::Clamp => self.index.saturating_sub(1),
        };
    }

    /// Pull the index back inside a list that changed under it.
    ///
    /// Call this whenever the backing list is replaced, otherwise a stale index
    /// silently points past the end.
    pub(crate) fn clamp(&mut self, len: usize) {
        self.index = if len == 0 { 0 } else { self.index.min(len - 1) };
    }

    pub(crate) fn reset(&mut self) {
        self.index = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_list_pins_the_cursor_at_zero() {
        let mut c = ListCursor::new();
        for overflow in [Overflow::Clamp, Overflow::Wrap] {
            c.next(0, overflow);
            assert_eq!(c.index(), 0);
            c.prev(0, overflow);
            assert_eq!(c.index(), 0);
        }
    }

    #[test]
    fn clamp_stops_at_the_ends() {
        let mut c = ListCursor::new();
        c.next(3, Overflow::Clamp);
        c.next(3, Overflow::Clamp);
        assert_eq!(c.index(), 2);
        c.next(3, Overflow::Clamp);
        assert_eq!(c.index(), 2, "must not move past the last item");

        c.prev(3, Overflow::Clamp);
        c.prev(3, Overflow::Clamp);
        assert_eq!(c.index(), 0);
        c.prev(3, Overflow::Clamp);
        assert_eq!(c.index(), 0, "must not move before the first item");
    }

    #[test]
    fn wrap_cycles_both_ways() {
        let mut c = ListCursor::new();
        c.prev(3, Overflow::Wrap);
        assert_eq!(c.index(), 2, "back from the first item goes to the last");
        c.next(3, Overflow::Wrap);
        assert_eq!(c.index(), 0, "forward from the last goes to the first");
    }

    #[test]
    fn selecting_out_of_range_lands_on_the_last_item() {
        let mut c = ListCursor::new();
        c.select(99, 4);
        assert_eq!(c.index(), 3);
        c.select(1, 4);
        assert_eq!(c.index(), 1);
        c.select(2, 0);
        assert_eq!(c.index(), 0, "nothing to select in an empty list");
    }

    #[test]
    fn a_shrinking_list_pulls_the_cursor_back() {
        let mut c = ListCursor::new();
        c.select(9, 10);
        assert_eq!(c.index(), 9);
        c.clamp(3);
        assert_eq!(c.index(), 2);
        c.clamp(0);
        assert_eq!(c.index(), 0);
    }
}
