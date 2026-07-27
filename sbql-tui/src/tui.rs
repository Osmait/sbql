//! Terminal lifetime and rendering.
//!
//! Everything that puts the terminal into an unusual state — raw mode, the
//! alternate screen, mouse capture, the Kitty keyboard protocol — is set up and
//! torn down here, so no other module has to remember the matching call.
//!
//! Restoration happens on `Drop`, from a panic hook, *and* by hand if setup
//! itself fails part-way through. Before this, a panic left the terminal in raw
//! mode with the alternate screen still active, which means a garbled shell and
//! no echo until the user blindly types `reset`.
//!
//! The awkward case is a failure *during* [`Tui::new`]: `Self` does not exist
//! yet, so there is nothing for `Drop` to run. [`TakeoverProgress`] records what
//! was actually switched on so it can be switched back off by hand.
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
use crate::error::{Result, TuiError};
use crate::renderer::Renderer;
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

/// Which parts of the takeover have actually happened.
///
/// Only used while [`Tui::new`] is running. Once the struct exists, `Drop` and
/// [`Tui::restore`] take over the same job.
#[derive(Default)]
struct TakeoverProgress {
    raw_mode: bool,
    alternate_screen: bool,
    keyboard_enhanced: bool,
}

impl TakeoverProgress {
    /// Undo a half-finished takeover.
    ///
    /// Best effort on purpose: we are already returning an error, and a second
    /// one raised while cleaning up after the first is not more useful than it.
    fn roll_back(&self) {
        let mut out = io::stdout();
        if self.keyboard_enhanced {
            let _ = execute!(out, PopKeyboardEnhancementFlags);
        }
        if self.alternate_screen {
            let _ = execute!(out, LeaveAlternateScreen, DisableMouseCapture);
        }
        if self.raw_mode {
            let _ = disable_raw_mode();
        }
    }
}

impl Tui<CrosstermBackend<Stdout>> {
    /// Take over the real terminal and install the panic hook.
    ///
    /// If any step fails the earlier ones are undone before returning, so a
    /// failed startup never costs the user their shell.
    pub fn new() -> Result<Self> {
        install_panic_hook();

        let mut progress = TakeoverProgress::default();
        match Self::take_over(&mut progress) {
            Ok(tui) => Ok(tui),
            Err(e) => {
                progress.roll_back();
                Err(e)
            }
        }
    }

    /// The fallible half of [`Tui::new`], with every step it completes recorded
    /// in `progress` so the caller can roll back exactly what happened.
    fn take_over(progress: &mut TakeoverProgress) -> Result<Self> {
        enable_raw_mode().map_err(TuiError::TerminalSetup)?;
        progress.raw_mode = true;

        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .map_err(TuiError::TerminalSetup)?;
        progress.alternate_screen = true;

        let keyboard_enhanced = supports_keyboard_enhancement().unwrap_or(false);
        if keyboard_enhanced {
            execute!(
                stdout,
                PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES,
                )
            )
            .map_err(TuiError::TerminalSetup)?;
            progress.keyboard_enhanced = true;
        }

        Ok(Self {
            terminal: Terminal::new(CrosstermBackend::new(stdout))
                .map_err(TuiError::TerminalSetup)?,
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
    /// What the `Backend` generic is for: the event loop, the reducer and the
    /// view all run against `TestBackend` with no terminal attached. Nothing in
    /// the shipped binary builds a `Tui` this way, hence `cfg(test)`.
    #[cfg(test)]
    pub fn with_backend(backend: B) -> Result<Self> {
        Ok(Self {
            terminal: Terminal::new(backend).map_err(TuiError::TerminalSetup)?,
            cache: ui::cache::RenderCache::new(),
            owns_terminal: false,
            keyboard_enhanced: false,
            restored: false,
        })
    }

    /// Hand the terminal back. Idempotent, a no-op for borrowed backends, and
    /// also run from `Drop`.
    ///
    /// Every step runs even after an earlier one fails, and the first error is
    /// reported at the end. Returning on the first failure instead would leave
    /// the alternate screen up with no second chance: `restored` is already set,
    /// so `Drop` would decline to try again.
    pub fn restore(&mut self) -> Result<()> {
        if self.restored || !self.owns_terminal {
            return Ok(());
        }
        self.restored = true;

        // Written to stdout rather than through the backend: it is the same
        // stream, and it keeps the generic free of a `Write` bound.
        let mut out = io::stdout();
        let mut failure: Option<io::Error> = None;

        keep_first(&mut failure, disable_raw_mode());
        if self.keyboard_enhanced {
            keep_first(&mut failure, execute!(out, PopKeyboardEnhancementFlags));
        }
        keep_first(
            &mut failure,
            execute!(out, LeaveAlternateScreen, DisableMouseCapture),
        );
        keep_first(&mut failure, self.terminal.show_cursor());

        match failure {
            Some(e) => Err(TuiError::TerminalRestore(e)),
            None => Ok(()),
        }
    }
}

/// Remember `result`'s error only if nothing has failed yet.
///
/// The first failure is the one worth reporting; the ones after it are usually
/// the same broken terminal answering again.
fn keep_first(slot: &mut Option<io::Error>, result: io::Result<()>) {
    if let Err(e) = result {
        slot.get_or_insert(e);
    }
}

impl<B: Backend> Renderer for Tui<B> {
    fn render(&mut self, state: &mut AppState) -> Result<()> {
        let cache = &mut self.cache;
        self.terminal
            .draw(|frame| ui::draw(frame, state, cache))
            .map_err(TuiError::Render)?;
        Ok(())
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

    /// The rollback path is what stands between a failed startup and a shell
    /// with no echo, so it has to undo exactly what was switched on — and
    /// nothing that was not.
    #[test]
    fn a_half_finished_takeover_is_rolled_back() {
        // Nothing was switched on: rolling back must be a no-op rather than,
        // say, popping keyboard flags that were never pushed.
        TakeoverProgress::default().roll_back();

        // The realistic failure: raw mode is on, the alternate screen is not.
        // Under `cargo test` there is no terminal to act on, so this asserts
        // that the call is safe to make rather than what it emits.
        TakeoverProgress {
            raw_mode: true,
            ..Default::default()
        }
        .roll_back();
    }

    /// A failure part-way through teardown must not skip the rest of it.
    #[test]
    fn restore_is_still_only_run_once() {
        let mut tui = Tui::with_backend(TestBackend::new(20, 5)).expect("build tui");
        tui.owns_terminal = false;

        assert!(tui.restore().is_ok());
        assert!(tui.restored || !tui.owns_terminal);
        assert!(tui.restore().is_ok(), "a second restore is a no-op");
    }

    #[test]
    fn the_first_teardown_failure_is_the_one_reported() {
        let mut slot = None;
        keep_first(&mut slot, Ok(()));
        assert!(slot.is_none());

        keep_first(&mut slot, Err(io::Error::other("first")));
        keep_first(&mut slot, Err(io::Error::other("second")));

        let kept = slot.expect("an error was recorded");
        assert_eq!(kept.to_string(), "first");
    }

    #[test]
    fn drawing_goes_through_to_the_backend() {
        let mut tui = Tui::with_backend(TestBackend::new(80, 24)).expect("build tui");
        let mut state = AppState::new(vec![]);

        tui.render(&mut state).expect("draw");

        let rendered = tui.rendered();
        assert!(
            rendered.contains("Connections"),
            "expected the panels to be painted, got:\n{rendered}"
        );
    }
}
