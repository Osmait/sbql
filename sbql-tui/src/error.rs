//! What can go wrong in the terminal layer.
//!
//! The application's *user-facing* errors — a query that failed, a connection
//! that refused — are not these. Those arrive from `sbql-core` as
//! [`CoreError`](sbql_core::CoreError) values and are painted into the status
//! bar; they never stop the program.
//!
//! [`TuiError`] is for the other kind: the run cannot continue, or has already
//! ended, and something has to be printed to a plain terminal. Each variant
//! names the thing that failed rather than the call that returned, because the
//! message is read by a user, not a developer — and because
//! [`TerminalRestore`](TuiError::TerminalRestore) in particular has to tell
//! them how to get their shell back.

use std::io;

use crate::cli::StartupError;

#[derive(Debug, thiserror::Error)]
pub(crate) enum TuiError {
    /// Raw mode, the alternate screen or the backend could not be set up.
    #[error("could not take over the terminal")]
    TerminalSetup(#[source] io::Error),

    /// The terminal could not be handed back. The shell is likely unusable.
    #[error("could not restore the terminal — run `reset` to fix your shell")]
    TerminalRestore(#[source] io::Error),

    /// A frame could not be painted.
    #[error("could not draw to the terminal")]
    Render(#[source] io::Error),

    /// The keyboard/mouse reader died, so no further input can arrive.
    #[error("lost the terminal input stream")]
    Input(#[source] io::Error),

    /// Something was wrong before the UI ever started.
    #[error(transparent)]
    Startup(#[from] StartupError),
}

pub(crate) type Result<T> = std::result::Result<T, TuiError>;

#[cfg(test)]
mod tests {
    use super::*;

    /// The cause is attached as a `source`, not flattened into the message, so
    /// callers can print either the summary or the whole chain.
    #[test]
    fn the_underlying_io_error_stays_reachable() {
        let err = TuiError::TerminalSetup(io::Error::new(io::ErrorKind::BrokenPipe, "no tty"));

        assert_eq!(err.to_string(), "could not take over the terminal");
        let source = std::error::Error::source(&err).expect("cause attached");
        assert!(source.to_string().contains("no tty"), "{source}");
    }

    /// A broken restore is the one error the user has to act on themselves.
    #[test]
    fn a_failed_restore_says_how_to_recover() {
        let err = TuiError::TerminalRestore(io::Error::other("nope"));
        assert!(err.to_string().contains("reset"), "{err}");
    }
}
