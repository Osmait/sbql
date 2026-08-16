//! Remembering which connection was open last.
//!
//! A small convenience, deliberately best-effort: every failure is ignored,
//! because not restoring the previous connection is never worth interrupting
//! the user over.

use std::path::PathBuf;

use uuid::Uuid;

/// `last-connection`, alongside the connection file so `SBQL_CONFIG_DIR`
/// relocates both together.
fn path() -> Option<PathBuf> {
    sbql_core::config_path()
        .ok()
        .map(|p| p.with_file_name("last-connection"))
}

/// The connection id from the previous run, if one was recorded.
pub(crate) fn last_connection_id() -> Option<String> {
    let raw = std::fs::read_to_string(path()?).ok()?;
    let trimmed = raw.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

pub(crate) fn remember(id: &Uuid) {
    if let Some(path) = path() {
        // Still ignored as far as the user is concerned (see the module docs),
        // but written to the log: "it forgot my connection again" is otherwise
        // a report with nothing behind it to look at.
        if let Err(e) = std::fs::write(path, id.to_string()) {
            tracing::debug!("could not record the last connection: {e}");
        }
    }
}

/// `theme`, alongside the connection file for the same reason.
fn theme_path() -> Option<PathBuf> {
    sbql_core::config_path()
        .ok()
        .map(|p| p.with_file_name("theme"))
}

/// The theme chosen in a previous run, if one was recorded.
pub(crate) fn last_theme() -> Option<String> {
    let raw = std::fs::read_to_string(theme_path()?).ok()?;
    let trimmed = raw.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

pub(crate) fn remember_theme(name: &str) {
    if let Some(path) = theme_path() {
        if let Err(e) = std::fs::write(path, name) {
            tracing::debug!("could not record the chosen theme: {e}");
        }
    }
}

pub(crate) fn forget() {
    if let Some(path) = path() {
        // `NotFound` is the ordinary case — disconnecting twice, or a first run
        // that never recorded anything — so this is a debug line, not a warning.
        if let Err(e) = std::fs::remove_file(path) {
            tracing::debug!("could not clear the last connection: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The chosen theme has to survive a restart, and an empty file must not
    /// be mistaken for a choice.
    #[test]
    fn a_theme_round_trips() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::env::set_var(sbql_core::CONFIG_DIR_ENV, dir.path());

        assert_eq!(last_theme(), None, "nothing chosen yet");
        remember_theme("Nord");
        assert_eq!(last_theme().as_deref(), Some("Nord"));

        // Whitespace is trimmed, and a blank file reads as no choice.
        remember_theme("   ");
        assert_eq!(last_theme(), None);
    }

    /// The file sits next to connections.toml, so pointing SBQL_CONFIG_DIR at a
    /// scratch directory isolates the session file too.
    #[test]
    fn remembering_and_forgetting_round_trips() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::env::set_var(sbql_core::CONFIG_DIR_ENV, dir.path());

        assert_eq!(last_connection_id(), None, "nothing recorded yet");

        let id = Uuid::new_v4();
        remember(&id);
        assert_eq!(last_connection_id(), Some(id.to_string()));

        forget();
        assert_eq!(last_connection_id(), None, "cleared on disconnect");
    }

    #[test]
    fn a_blank_file_counts_as_nothing_recorded() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::env::set_var(sbql_core::CONFIG_DIR_ENV, dir.path());
        std::fs::write(path().unwrap(), "   \n").unwrap();

        assert_eq!(last_connection_id(), None);
    }
}
