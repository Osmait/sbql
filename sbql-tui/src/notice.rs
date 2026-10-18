//! What the status bar has to say, and for how long.
//!
//! There used to be two fields — `status_msg` and `error_msg` — and a view that
//! arbitrated between them by always preferring the error. That is why
//! disconnecting appeared to do nothing: `Disconnected` set the status, an
//! error from ten minutes earlier was still sitting in the other field, and the
//! bar went on showing the error. One field cannot disagree with itself.
//!
//! It also carries what a bare `String` could not: how bad it is, what the user
//! might do about it, and the part of the message too long for one line.

use sbql_core::{CoreError, ErrorKind, Severity};

/// The clock runs on the 100 ms UI tick, so these are tenths of a second.
const TICKS_PER_SECOND: u64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Level {
    /// It worked. Says so briefly and gets out of the way.
    Info,
    /// It worked, but not completely.
    Warning,
    /// It did not work.
    Error,
}

/// One thing to tell the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Notice {
    pub level: Level,
    /// One line, for the status bar.
    pub text: String,
    /// The rest of it. Too long for the bar, so it lives behind Ctrl+E.
    pub detail: Option<String>,
    /// Set when this came from the core, and the reason a `CoreError` is worth
    /// having: it is what [`Notice::hint`] reads.
    pub kind: Option<ErrorKind>,
    /// The tick this was posted on. Compared against the current tick to expire
    /// it — a wall clock would be one more thing for tests to control.
    pub posted_at: u64,
}

impl Notice {
    pub(crate) fn info(text: impl Into<String>, now: u64) -> Self {
        Self {
            level: Level::Info,
            text: text.into(),
            detail: None,
            kind: None,
            posted_at: now,
        }
    }

    /// A failure the TUI decided on by itself, with no core error behind it.
    pub(crate) fn error(text: impl Into<String>, now: u64) -> Self {
        Self {
            level: Level::Error,
            ..Self::info(text, now)
        }
    }

    /// A failure — or a caveat — reported by `sbql-core`.
    pub(crate) fn from_core(err: CoreError, now: u64) -> Self {
        Self {
            level: match err.severity {
                Severity::Warning => Level::Warning,
                Severity::Error => Level::Error,
                // `Severity` is `#[non_exhaustive]`, so this arm is required.
                // It resolves to `Error` rather than `Warning` deliberately: a
                // severity this build does not recognise is more likely to
                // matter than not, and a warning toast is dismissable enough
                // that the user can miss it. Fail loud on the unknown.
                _ => Level::Error,
            },
            text: err.message,
            detail: err.detail,
            kind: Some(err.kind),
            posted_at: now,
        }
    }

    /// How long this should stay on screen, in ticks.
    ///
    /// Failures do not expire: the user has to be able to look away and come
    /// back to a query that did not run. They go when something replaces them
    /// or when Esc dismisses them. Everything else clears itself, so a stale
    /// "Connected to prod" does not sit on top of the help line all session.
    fn lifetime(&self) -> Option<u64> {
        match self.level {
            Level::Info => Some(4 * TICKS_PER_SECOND),
            Level::Warning => Some(12 * TICKS_PER_SECOND),
            Level::Error => None,
        }
    }

    /// Whether `now` is past this notice's welcome.
    pub(crate) fn is_expired(&self, now: u64) -> bool {
        match self.lifetime() {
            Some(ticks) => now.saturating_sub(self.posted_at) >= ticks,
            None => false,
        }
    }

    /// Whether there is more to read than the bar is showing.
    pub(crate) fn has_detail(&self) -> bool {
        self.detail.is_some() || self.hint().is_some()
    }

    /// What the user can do about it.
    ///
    /// The whole point of [`ErrorKind`] travelling out of the core: the bar
    /// used to be able to print the message and nothing else, so "No active
    /// connection" left the user to work out that connections open with Enter.
    pub(crate) fn hint(&self) -> Option<&'static str> {
        match self.kind? {
            ErrorKind::NoActiveConnection => {
                Some("Pick a connection in the Connections panel (F1) and press Enter to open it.")
            }
            ErrorKind::Connection => {
                Some("Check the host, port and credentials with `e`, then try connecting again.")
            }
            ErrorKind::Query => {
                Some("Fix the statement in the editor (F3) and run it again with Ctrl+S.")
            }
            ErrorKind::Config => Some("Edit the connection with `e` to correct it."),
            ErrorKind::Credentials => Some(
                "Re-enter the password with `e`, or start sbql with --no-keyring to keep it \
                      in memory for this session.",
            ),
            // Nothing useful to suggest — better to say nothing than to guess.
            ErrorKind::Io | ErrorKind::Other => None,
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_core_warning_is_not_reported_as_a_failure() {
        let warning = CoreError::warning(ErrorKind::Credentials, "saved, password not stored")
            .with_detail("no Secret Service running");
        let notice = Notice::from_core(warning, 0);

        assert_eq!(notice.level, Level::Warning);
        assert_eq!(notice.detail.as_deref(), Some("no Secret Service running"));
    }

    /// The bug this type exists to make impossible: something the user must act
    /// on quietly disappearing, or something trivial never leaving.
    #[test]
    fn failures_stay_and_confirmations_do_not() {
        let ok = Notice::info("Connected to prod", 0);
        let bad = Notice::from_core(CoreError::new(ErrorKind::Query, "syntax error"), 0);

        assert!(!ok.is_expired(0));
        assert!(ok.is_expired(10_000), "a confirmation should clear itself");
        assert!(
            !bad.is_expired(10_000),
            "a failure must wait for the user, not a timer"
        );
    }

    #[test]
    fn a_warning_outlives_a_confirmation() {
        let info = Notice::info("saved", 0);
        let warn = Notice::from_core(CoreError::warning(ErrorKind::Credentials, "partly"), 0);

        // Long enough for the confirmation to have gone, short enough that the
        // warning is still up.
        let now = 5 * TICKS_PER_SECOND;
        assert!(info.is_expired(now));
        assert!(!warn.is_expired(now));
    }

    #[test]
    fn the_hint_follows_the_kind() {
        let no_conn = Notice::from_core(
            CoreError::new(ErrorKind::NoActiveConnection, "No active connection"),
            0,
        );
        assert!(no_conn.hint().expect("a hint").contains("Enter"));

        let query = Notice::from_core(CoreError::new(ErrorKind::Query, "bad sql"), 0);
        assert!(query.hint().expect("a hint").contains("editor"));

        // A message we wrote ourselves has no kind, so it gets no invented hint.
        assert_eq!(Notice::error("something", 0).hint(), None);
    }

    #[test]
    fn a_notice_knows_whether_there_is_more_to_show() {
        assert!(!Notice::info("done", 0).has_detail());

        let with_cause = Notice::from_core(
            CoreError::new(ErrorKind::Io, "could not write").with_detail("permission denied"),
            0,
        );
        assert!(with_cause.has_detail());

        // A hint counts as something more to read, even with no cause attached.
        let hinted = Notice::from_core(CoreError::new(ErrorKind::Query, "bad sql"), 0);
        assert_eq!(hinted.detail, None);
        assert!(hinted.has_detail());
    }
}
