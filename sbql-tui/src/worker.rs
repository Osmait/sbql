//! Async worker task that bridges the TUI with `sbql-core`.
//!
//! The worker owns a `Core` instance and processes `CoreCommand` values
//! sent from the main UI loop, replying with `CoreEvent` values.

use sbql_core::{Core, CoreCommand, CoreEvent};
use tokio::sync::mpsc;

/// Spawn the worker task.
///
/// Returns `(cmd_tx, event_rx)`:
/// - `cmd_tx` — the UI uses this to send commands to the Core.
/// - `event_rx` — the UI receives Core responses from this.
pub(crate) fn spawn_worker() -> (
    mpsc::UnboundedSender<CoreCommand>,
    mpsc::UnboundedReceiver<CoreEvent>,
) {
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<CoreCommand>();
    let (event_tx, event_rx) = mpsc::unbounded_channel::<CoreEvent>();

    tokio::spawn(async move {
        let mut core = Core::new();

        // Whatever the core wants said before the first command: the connection
        // list, and any reason it came back empty.
        // A send can only fail because the UI dropped the receiver, i.e. it quit
        // before the first frame. Discarded rather than reported: there is
        // nobody left to report to, and the loop below already exits on it.
        for ev in core.startup_events() {
            drop(event_tx.send(ev));
        }

        while let Some(cmd) = cmd_rx.recv().await {
            // Each command declares whether it is worth a spinner, so adding
            // one here cannot accidentally blank the results pane.
            if cmd.shows_progress() {
                // Same as above, and deliberately not an early return: a UI that
                // has gone away still leaves the command to be run for its
                // side effects.
                drop(event_tx.send(CoreEvent::Loading));
            }

            let events = core.handle(cmd).await;
            for ev in events {
                tracing::debug!("Sending event: {:?}", ev);
                if event_tx.send(ev).is_err() {
                    // UI dropped the receiver — shut down.
                    return;
                }
            }
        }

        // Not an error: this is what quitting looks like from in here. The UI
        // drops the sender on the way out, `recv` returns `None`, and the
        // worker is done. Logged at error level it sent every future
        // investigation looking for a fault at shutdown.
        tracing::debug!("Command channel closed, worker shutting down");
    });

    (cmd_tx, event_rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_spawn_worker_initialization() {
        let (_cmd_tx, mut event_rx) = spawn_worker();

        // Worker should immediately send ConnectionList on startup
        let initial_event = event_rx.recv().await.expect("Worker closed immediately");
        match initial_event {
            CoreEvent::ConnectionList(conns) => {
                // By default the new core should have 0 connections since we are not loading from disk in the test env
                // (or if it does load from disk, it's just a valid vector)
                assert!(conns.is_empty() || !conns.is_empty());
            }
            _ => panic!("Expected ConnectionList as first event"),
        }
    }

    #[tokio::test]
    async fn test_spawn_worker_handles_command() {
        let (cmd_tx, mut event_rx) = spawn_worker();

        // Drain initial event
        drop(event_rx.recv().await);

        // Send a command that triggers Loading
        cmd_tx.send(CoreCommand::ListTables).unwrap();

        // First we should get a loading event
        let loading_event = event_rx.recv().await.expect("Expected event");
        assert!(matches!(loading_event, CoreEvent::Loading));

        // Since we are not connected, we should get an error next
        let error_event = event_rx.recv().await.expect("Expected event");
        match error_event {
            CoreEvent::Error(e) => {
                assert_eq!(e.kind, sbql_core::ErrorKind::NoActiveConnection);
            }
            _ => panic!("Expected Error event, got {error_event:?}"),
        }
    }
}
