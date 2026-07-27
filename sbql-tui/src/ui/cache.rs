//! Memoised render output, owned by the view.
//!
//! Syntax highlighting and the diagram canvas are expensive enough to be worth
//! keeping between frames, but they are *rendered* values — spans, styles,
//! ratatui `Line`s. They used to be stored on `AppState`, which meant the
//! model imported the view's vocabulary and reducers held onto rendering
//! decisions they had no business knowing about.
//!
//! They live here instead. The model keeps only the cheap facts the cache is
//! keyed on: a revision counter for the editor, a dirty flag for the diagram.
//! Nothing here affects behaviour — dropping the whole cache would only cost
//! time.

use ratatui::style::Style;
use ratatui::text::Line;

/// One line of source split into styled runs.
pub type HighlightedLine = Vec<(Style, String)>;

#[derive(Default)]
pub struct RenderCache {
    highlight: Option<Vec<HighlightedLine>>,
    /// Editor revision the highlight was built from.
    highlight_revision: Option<u64>,
    diagram_canvas: Option<Vec<Line<'static>>>,
}

impl RenderCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Highlighted source for `revision`, rebuilding with `build` when the
    /// editor has moved on since the cached copy.
    pub fn highlight(
        &mut self,
        revision: u64,
        build: impl FnOnce() -> Vec<HighlightedLine>,
    ) -> &[HighlightedLine] {
        if self.highlight_revision != Some(revision) || self.highlight.is_none() {
            self.highlight = Some(build());
            self.highlight_revision = Some(revision);
        }
        self.highlight.as_deref().unwrap_or_default()
    }

    /// The diagram canvas, if one has been built and not invalidated since.
    pub fn diagram_canvas(&self) -> Option<&[Line<'static>]> {
        self.diagram_canvas.as_deref()
    }

    pub fn set_diagram_canvas(&mut self, lines: Vec<Line<'static>>) {
        self.diagram_canvas = Some(lines);
    }

    /// Forget the diagram canvas — call when the diagram closes so a stale one
    /// cannot be shown by the next.
    pub fn clear_diagram_canvas(&mut self) {
        self.diagram_canvas = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn highlight_is_reused_until_the_revision_moves() {
        let mut cache = RenderCache::new();
        let builds = Cell::new(0);
        let build_for = |cache: &mut RenderCache, rev: u64| {
            cache.highlight(rev, || {
                builds.set(builds.get() + 1);
                vec![vec![(Style::default(), format!("rev{rev}"))]]
            });
        };

        build_for(&mut cache, 1);
        build_for(&mut cache, 1);
        assert_eq!(builds.get(), 1, "same revision must not rebuild");

        build_for(&mut cache, 2);
        assert_eq!(builds.get(), 2, "a new revision rebuilds");
    }

    #[test]
    fn highlight_returns_what_was_built() {
        let mut cache = RenderCache::new();
        let lines = cache.highlight(7, || vec![vec![(Style::default(), "select".into())]]);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0][0].1, "select");
    }

    #[test]
    fn the_diagram_canvas_can_be_dropped() {
        let mut cache = RenderCache::new();
        assert!(cache.diagram_canvas().is_none());

        cache.set_diagram_canvas(vec![Line::from("a"), Line::from("b")]);
        assert_eq!(cache.diagram_canvas().map(<[_]>::len), Some(2));

        cache.clear_diagram_canvas();
        assert!(
            cache.diagram_canvas().is_none(),
            "stale canvas must not survive"
        );
    }
}
