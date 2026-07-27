//! What the application needs from a display.
//!
//! [`Sbql`](crate::sbql::Sbql) drives an event loop; painting is the only thing
//! it asks the outside world to do for it. Naming that as a trait means the
//! application layer never mentions ratatui — not even to name a generic bound,
//! which is what it used to do by taking `Tui<B: ratatui::backend::Backend>`.
//!
//! The seam is deliberately one method wide. Anything larger would be inventing
//! a UI framework rather than describing a dependency.
//!
//! ## What this does and does not buy
//!
//! It makes the application independent of *how* the terminal is painted:
//! ratatui today, a recorded transcript in tests, a plain-text or HTML dump, a
//! renderer that draws nothing at all.
//!
//! It does not make the application independent of being a terminal app. The
//! state it hands over is [`AppState`], which is full of focused panels, scroll
//! offsets and vim modes. A GUI client would not consume that — it shares
//! `sbql-core` instead, which is the boundary that carries no UI at all.

use crate::app::AppState;
use crate::error::Result;

/// A surface the application can paint its state onto.
pub trait Renderer {
    /// Paint the current state.
    ///
    /// Takes `&mut AppState` because measuring is part of painting: viewport
    /// sizes and cached geometry are settled here and read back by scrolling
    /// and paging.
    ///
    /// A failure here ends the run: if a frame cannot be painted there is
    /// nowhere left to report anything, so the error travels up to `main`,
    /// which prints it to a restored terminal.
    fn render(&mut self, state: &mut AppState) -> Result<()>;
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    /// A renderer that records what it was asked to paint, and draws nothing.
    ///
    /// Exists to show the seam holds: the event loop runs to completion with no
    /// ratatui, no terminal, and no backend of any kind.
    #[derive(Default)]
    pub(crate) struct RecordingRenderer {
        /// One entry per frame: the mode the app was in when it was painted.
        pub frames: Vec<crate::app::Mode>,
    }

    impl Renderer for RecordingRenderer {
        fn render(&mut self, state: &mut AppState) -> Result<()> {
            self.frames.push(state.mode());
            Ok(())
        }
    }
}
