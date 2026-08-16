//! Entry point.
//!
//! Deliberately thin: parse the command line, start logging, take over the
//! terminal, hand control to [`Sbql`]. Everything else lives in a module that
//! can be tested without a terminal attached.

use std::error::Error;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

mod action;
mod app;
mod cli;
mod completion;
mod error;
mod events;
mod handlers;
mod highlight;
mod list_cursor;
mod notice;
mod renderer;
mod sbql;
mod session;
#[cfg(test)]
mod test_helpers;
mod tui;
mod ui;
mod worker;

use cli::Cli;
use error::Result;
use sbql::Sbql;
use tui::Tui;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            report(&mut io::stderr(), &e);
            ExitCode::FAILURE
        }
    }
}

/// Print an error and everything underneath it, one line each.
///
/// `main` deliberately does not return `Result`. That prints the `Debug` form,
/// which for a `TuiError` reads `TerminalRestore(Custom { kind: Other, .. })` —
/// and buries the one line that matters, the one telling the user to run
/// `reset`. Writing it out by hand costs nothing and shows the whole chain.
fn report(out: &mut impl Write, err: &dyn Error) {
    // Discarding the write errors on purpose: this *is* the error path, and if
    // the stream we report on is itself broken there is nowhere left to say so.
    drop(writeln!(out, "sbql: {err}"));
    let mut source = err.source();
    while let Some(cause) = source {
        drop(writeln!(out, "  caused by: {cause}"));
        source = cause.source();
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    cli.apply_env();

    // Resolve the requested connection before raw mode, so a bad name is
    // readable instead of being swallowed by the alternate screen.
    let startup_connection = cli.startup_connection()?;

    init_logging();

    let mut tui = Tui::new()?;
    let mut app = Sbql::new(startup_connection);
    let result = app.run(&mut tui).await;

    // Restore explicitly so a failure to hand the terminal back is reported,
    // rather than being swallowed by `Drop`.
    match (result, tui.restore()) {
        (Err(app_err), Err(restore_err)) => {
            // The run failed *and* the terminal is now in an unknown state. The
            // run's own error is the one that explains what happened, so that
            // is what we return — but the user still has to be told their shell
            // may need `reset`, which the returned error would not say.
            //
            // Printing is safe *here* and nowhere else in this crate: the
            // alternate screen has just been given back, so this lands in the
            // user's shell rather than in the middle of a frame.
            #[allow(clippy::print_stderr)]
            {
                eprintln!("sbql: {restore_err}");
            }
            Err(app_err)
        }
        (Err(e), Ok(())) | (Ok(()), Err(e)) => Err(e),
        (Ok(()), Ok(())) => Ok(()),
    }
}

/// Log to a file: anything on stdout or stderr would corrupt the display.
///
/// Best effort, and deliberately not fatal. Losing the log is a nuisance;
/// refusing to start a database client because a log file could not be opened
/// is worse. The reason is printed instead — this runs before the terminal is
/// taken over, so stderr is still the user's.
fn init_logging() {
    match try_init_logging() {
        Ok(path) => tracing::debug!("logging to {}", path.display()),
        // Printing is safe *here* and nowhere else in this crate: this runs
        // before the terminal is taken over, so it lands in the user's shell
        // rather than in the middle of a frame.
        #[allow(clippy::print_stderr)]
        Err(e) => eprintln!("sbql: logging disabled: {e}"),
    }
}

fn try_init_logging() -> io::Result<PathBuf> {
    let path = log_path()?;
    let file = std::fs::File::create(&path)?;
    // `RUST_LOG` wins outright when it is set. This used to append fixed
    // `sbql_*=info` directives on top of it, which meant those directives
    // always beat anything the user asked for — `RUST_LOG=sbql_tui=debug`
    // could not turn on debug logging for the one crate you would want it for.
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("sbql_core=info,sbql_tui=info"));

    tracing_subscriber::fmt()
        .with_writer(std::sync::Mutex::new(std::io::LineWriter::new(file)))
        .with_env_filter(filter)
        .init();
    Ok(path)
}

/// Where the log goes: `$SBQL_LOG` if set, otherwise a per-user state or cache
/// directory.
///
/// Not `/tmp/sbql.log`, which this used to be. That path is shared, predictable
/// and world-writable: on a multi-user machine anyone can create it first — as
/// a symlink to a file of ours, say — and `File::create` would follow it and
/// truncate the target.
fn log_path() -> io::Result<PathBuf> {
    if let Some(explicit) = std::env::var_os("SBQL_LOG") {
        return Ok(PathBuf::from(explicit));
    }

    let base = dirs::state_dir().or_else(dirs::cache_dir).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "no state or cache directory for this user; set SBQL_LOG to choose one",
        )
    })?;

    let dir = base.join("sbql");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("sbql.log"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use error::TuiError;

    fn reported(err: &dyn Error) -> String {
        let mut out = Vec::new();
        report(&mut out, err);
        String::from_utf8(out).expect("utf-8")
    }

    /// The message the user reads has to be the `Display` one. `TerminalRestore`
    /// exists to tell them their shell needs `reset`; the `Debug` form that
    /// `fn main() -> Result` would print does not say that.
    #[test]
    fn a_failed_restore_tells_the_user_what_to_do() {
        let err = TuiError::TerminalRestore(io::Error::other("ioctl refused"));
        let out = reported(&err);

        assert!(out.contains("reset"), "{out}");
        assert!(
            !out.contains("TerminalRestore"),
            "Debug form leaked:\n{out}"
        );
    }

    /// The cause chain is why `TuiError` attaches sources instead of formatting
    /// them into the message.
    #[test]
    fn the_cause_is_printed_under_the_message() {
        let err =
            TuiError::TerminalSetup(io::Error::new(io::ErrorKind::BrokenPipe, "not a terminal"));
        let out = reported(&err);

        assert!(
            out.starts_with("sbql: could not take over the terminal"),
            "{out}"
        );
        assert!(out.contains("caused by: not a terminal"), "{out}");
    }

    /// A bad connection name resolves before the terminal is touched, and its
    /// message is the useful one rather than a wrapper's.
    #[test]
    fn a_startup_problem_reads_as_itself() {
        let err = TuiError::Startup(cli::StartupError::UnknownConnection {
            requested: "prod".into(),
            available: vec!["dev".into()],
        });
        let out = reported(&err);

        assert!(out.contains("'prod' not found"), "{out}");
        assert!(out.contains("dev"), "{out}");
    }
}
