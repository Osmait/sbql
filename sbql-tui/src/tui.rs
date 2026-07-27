//! Terminal lifetime and rendering.
//!
//! Everything that puts the terminal into an unusual state — raw mode, the
//! alternate screen, mouse capture, the Kitty keyboard protocol — is set up and
//! torn down here, so no other module has to remember the matching call.
//!
//! Restoration happens on `Drop` *and* from a panic hook. Before this, a panic
//! left the terminal in raw mode with the alternate screen still active, which
//! means a garbled shell and no echo until the user blindly types `reset`.
//!
//! [`Tui`] is generic over ratatui's own [`Backend`] trait rather than nailed to
//! crossterm. That is not speculative: it is what lets the event loop be driven
//! end-to-end in a test against `TestBackend`, with no terminal attached.

use std::io::{self, Stdout};

use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, supports_keyboard_enhancement, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use ratatui::{backend::Backend, backend::CrosstermBackend, Terminal};

use crate::app::AppState;
use crate::ui;

/// The terminal, plus the render state that lives alongside it.
pub struct Tui<B: Backend> {
    terminal: Terminal<B>,
    /// Rendered output kept between frames. A rendering concern with a
    /// rendering lifetime, so it belongs here rather than on the application.
    cache: ui::cache::RenderCache,
    /// Whether we actually took a real terminal over and owe it a restore.
    owns_terminal: bool,
    /// Whether the Kitty keyboard protocol was pushed and must be popped.
    keyboard_enhanced: bool,
    /// Guards against restoring twice, once explicitly and once on drop.
    restored: bool,
}

impl Tui<CrosstermBackend<Stdout>> {
    /// Take over the real terminal and install the panic hook.
    pub fn new() -> io::Result<Self> {
        install_panic_hook();

        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

        let keyboard_enhanced = supports_keyboard_enhancement().unwrap_or(false);
        if keyboard_enhanced {
            execute!(
                stdout,
                PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES,
                )
            )?;
        }

        Ok(Self {
            terminal: Terminal::new(CrosstermBackend::new(stdout))?,
            cache: ui::cache::RenderCache::new(),
            owns_terminal: true,
            keyboard_enhanced,
            restored: false,
        })
    }
}

impl<B: Backend> Tui<B> {
    /// Wrap a backend without touching the real terminal.
    ///
    /// For tests, and for any backend that manages its own lifetime.
    pub fn with_backend(backend: B) -> io::Result<Self> {
        Ok(Self {
            terminal: Terminal::new(backend)?,
            cache: ui::cache::RenderCache::new(),
            owns_terminal: false,
            keyboard_enhanced: false,
            restored: false,
        })
    }

    /// Paint the current state.
    pub fn draw(&mut self, state: &mut AppState) -> io::Result<()> {
        let cache = &mut self.cache;
        self.terminal.draw(|frame| ui::draw(frame, state, cache))?;
        Ok(())
    }

    /// Hand the terminal back. Idempotent, a no-op for borrowed backends, and
    /// also run from `Drop`.
    pub fn restore(&mut self) -> io::Result<()> {
        if self.restored || !self.owns_terminal {
            return Ok(());
        }
        self.restored = true;

        // Written to stdout rather than through the backend: it is the same
        // stream, and it keeps the generic free of a `Write` bound.
        let mut out = io::stdout();
        disable_raw_mode()?;
        if self.keyboard_enhanced {
            execute!(out, PopKeyboardEnhancementFlags)?;
        }
        execute!(out, LeaveAlternateScreen, DisableMouseCapture)?;
        self.terminal.show_cursor()
    }
}

#[cfg(test)]
impl Tui<ratatui::backend::TestBackend> {
    /// Everything currently on the test screen, as one string.
    pub(crate) fn rendered(&self) -> String {
        self.terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }
}

impl<B: Backend> Drop for Tui<B> {
    fn drop(&mut self) {
        // Nothing useful to do with an error while unwinding.
        let _ = self.restore();
    }
}

/// Put the terminal back before the default panic message is printed.
///
/// Without this the message is written into the alternate screen with raw mode
/// still on, so it scrolls past unreadably and the shell is left broken.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    #[test]
    fn a_borrowed_backend_is_never_restored() {
        let mut tui = Tui::with_backend(TestBackend::new(20, 5)).expect("build tui");
        // Would otherwise disable raw mode on a terminal we never took over.
        assert!(tui.restore().is_ok());
        assert!(!tui.owns_terminal);
    }

    #[test]
    fn drawing_goes_through_to_the_backend() {
        let mut tui = Tui::with_backend(TestBackend::new(80, 24)).expect("build tui");
        let mut state = AppState::new(vec![]);

        tui.draw(&mut state).expect("draw");

        let rendered = tui.rendered();
        assert!(
            rendered.contains("Connections"),
            "expected the panels to be painted, got:\n{rendered}"
        );
    }
}
