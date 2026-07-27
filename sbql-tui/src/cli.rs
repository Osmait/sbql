//! Command-line surface.
//!
//! Kept apart from the event loop so the shape of the CLI is declared in one
//! place, and so `main` never parses arguments by hand.

use clap::Parser;

/// Terminal SQL workspace.
#[derive(Debug, Parser)]
#[command(
    name = "sbql",
    version,
    about = "Terminal SQL workspace",
    after_help = "ENVIRONMENT:\n  \
        SBQL_CONFIG_DIR  Directory holding connections.toml (default ~/.config/sbql)\n  \
        SBQL_NO_KEYRING  Set to 1 for the same effect as --no-keyring"
)]
pub struct Cli {
    /// Saved connection to open on startup.
    pub connection: Option<String>,

    /// Do not touch the OS credential store.
    ///
    /// Passwords are kept in memory for this session only and never written to
    /// disk. Useful when no Secret Service or Keychain is available.
    #[arg(long)]
    pub no_keyring: bool,
}

impl Cli {
    /// Apply the settings that other crates read from the environment.
    ///
    /// `sbql-core` checks `SBQL_NO_KEYRING` on every credential operation, so
    /// the flag has to be visible before anything touches the store.
    pub fn apply_env(&self) {
        if self.no_keyring {
            std::env::set_var(sbql_core::NO_KEYRING_ENV, "1");
        }
    }

    /// Resolve the requested connection name against what is actually saved.
    ///
    /// Done before the terminal switches to raw mode so the error prints
    /// legibly instead of being swallowed by the alternate screen.
    pub fn startup_connection(&self) -> Result<Option<String>, StartupError> {
        let Some(name) = self.connection.as_deref() else {
            return Ok(None);
        };

        let saved = sbql_core::load_connections().unwrap_or_default();
        if saved.iter().any(|c| c.name.eq_ignore_ascii_case(name)) {
            return Ok(Some(name.to_string()));
        }

        Err(StartupError::UnknownConnection {
            requested: name.to_string(),
            available: saved.into_iter().map(|c| c.name).collect(),
        })
    }
}

/// A problem worth reporting before the UI starts.
#[derive(Debug)]
pub enum StartupError {
    UnknownConnection {
        requested: String,
        available: Vec<String>,
    },
}

impl std::fmt::Display for StartupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StartupError::UnknownConnection {
                requested,
                available,
            } if available.is_empty() => write!(
                f,
                "no saved connections found, so '{requested}' cannot be opened.\n\
                 Start sbql with no arguments and press `n` to add one."
            ),
            StartupError::UnknownConnection {
                requested,
                available,
            } => {
                writeln!(f, "connection '{requested}' not found.")?;
                write!(f, "Available connections:")?;
                for name in available {
                    write!(f, "\n  {name}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for StartupError {}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn the_cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn a_bare_invocation_opens_nothing_in_particular() {
        let cli = Cli::parse_from(["sbql"]);
        assert_eq!(cli.connection, None);
        assert!(!cli.no_keyring);
    }

    #[test]
    fn a_connection_name_is_positional() {
        let cli = Cli::parse_from(["sbql", "staging"]);
        assert_eq!(cli.connection.as_deref(), Some("staging"));
    }

    #[test]
    fn the_keyring_can_be_turned_off_alongside_a_connection() {
        let cli = Cli::parse_from(["sbql", "--no-keyring", "staging"]);
        assert!(cli.no_keyring);
        assert_eq!(cli.connection.as_deref(), Some("staging"));
    }

    #[test]
    fn an_unknown_flag_is_rejected() {
        assert!(Cli::try_parse_from(["sbql", "--nope"]).is_err());
    }

    #[test]
    fn a_second_positional_is_rejected() {
        assert!(Cli::try_parse_from(["sbql", "one", "two"]).is_err());
    }

    #[test]
    fn an_unknown_connection_lists_what_is_available() {
        let err = StartupError::UnknownConnection {
            requested: "prod".into(),
            available: vec!["dev".into(), "staging".into()],
        };
        let msg = err.to_string();
        assert!(msg.contains("'prod' not found"), "{msg}");
        assert!(msg.contains("dev") && msg.contains("staging"), "{msg}");
    }

    #[test]
    fn with_nothing_saved_the_message_says_how_to_add_one() {
        let err = StartupError::UnknownConnection {
            requested: "prod".into(),
            available: vec![],
        };
        assert!(err.to_string().contains("press `n`"), "{err}");
    }
}
