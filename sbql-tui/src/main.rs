//! Entry point.
//!
//! Deliberately thin: parse the command line, start logging, take over the
//! terminal, hand control to [`Sbql`]. Everything else lives in a module that
//! can be tested without a terminal attached.

use anyhow::Result;
use clap::Parser;

mod action;
mod app;
mod cli;
mod completion;
mod events;
mod handlers;
mod highlight;
mod list_cursor;
mod renderer;
mod sbql;
mod session;
#[cfg(test)]
mod test_helpers;
mod tui;
mod ui;
mod worker;

use cli::Cli;
use sbql::Sbql;
use tui::Tui;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    cli.apply_env();

    // Resolve the requested connection before raw mode, so a bad name is
    // readable instead of being swallowed by the alternate screen.
    let startup_connection = match cli.startup_connection() {
        Ok(name) => name,
        Err(e) => {
            eprintln!("sbql: {e}");
            std::process::exit(1);
        }
    };

    init_logging()?;

    let mut tui = Tui::new()?;
    let mut app = Sbql::new(startup_connection);
    let result = app.run(&mut tui).await;

    // Restore explicitly so a failure to hand the terminal back is reported,
    // rather than being swallowed by `Drop`.
    tui.restore()?;
    result
}

/// Log to a file: anything on stdout or stderr would corrupt the display.
fn init_logging() -> Result<()> {
    let file = std::fs::File::create("/tmp/sbql.log")?;
    tracing_subscriber::fmt()
        .with_writer(std::sync::Mutex::new(std::io::LineWriter::new(file)))
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("sbql_core=info".parse()?)
                .add_directive("sbql_tui=info".parse()?),
        )
        .init();
    Ok(())
}
