//! Terminal lifetime.
//!
//! Everything that puts the terminal into an unusual state — raw mode, the
//! alternate screen, mouse capture, the Kitty keyboard protocol — is set up and
//! torn down here, so no other module has to remember the matching call.
//!
//! Restoration happens on `Drop` *and* from a panic hook. Before this, a panic
//! left the terminal in raw mode with the alternate screen still active, which
//! means a garbled shell and no echo until the user blindly types `reset`.

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
use ratatui::{backend::CrosstermBackend, Terminal};

pub type Backend = CrosstermBackend<Stdout>;

/// An initialised terminal that puts itself back the way it found it.
pub struct Tui {
    pub terminal: Terminal<Backend>,
    /// Whether the Kitty keyboard protocol was pushed and must be popped.
    keyboard_enhanced: bool,
    /// Guards against restoring twice, once explicitly and once on drop.
    restored: bool,
}

impl Tui {
    /// Take over the terminal and install the panic hook.
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
            keyboard_enhanced,
            restored: false,
        })
    }

    /// Hand the terminal back. Idempotent, and also run from `Drop`.
    pub fn restore(&mut self) -> io::Result<()> {
        if self.restored {
            return Ok(());
        }
        self.restored = true;

        disable_raw_mode()?;
        if self.keyboard_enhanced {
            execute!(self.terminal.backend_mut(), PopKeyboardEnhancementFlags)?;
        }
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        self.terminal.show_cursor()
    }
}

impl Drop for Tui {
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
