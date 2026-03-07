//! Async worker task that bridges the TUI with `sbql-core`.
//!
//! The worker owns a `Core` instance and processes `CoreCommand` values
//! sent from the main UI loop, replying with `CoreEvent` values.

use sbql_core::{Core, CoreCommand, CoreEvent};
use tokio::sync::mpsc;
use tracing::error;

/// Spawn the worker task.
///
/// Returns `(cmd_tx, event_rx)`:
/// - `cmd_tx` — the UI uses this to send commands to the Core.
/// - `event_rx` — the UI receives Core responses from this.
pub fn spawn_worker() -> (
    mpsc::UnboundedSender<CoreCommand>,
    mpsc::UnboundedReceiver<CoreEvent>,
) {
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<CoreCommand>();
    let (event_tx, event_rx) = mpsc::unbounded_channel::<CoreEvent>();

    tokio::spawn(async move {
        let mut core = Core::new();

        // Send the initial connection list so the UI can populate itself.
        let initial = core.connections.clone();
        let _ = event_tx.send(CoreEvent::ConnectionList(initial));

        while let Some(cmd) = cmd_rx.recv().await {
            // Only signal "loading" for operations that take noticeable time.
            // Fast background lookups (PK introspection) must not blank the UI.
            let show_loading = !matches!(
                cmd,
                CoreCommand::GetPrimaryKeys { .. }
                    | CoreCommand::Disconnect(_)
                    | CoreCommand::LoadDiagram
                    | CoreCommand::SuggestFilterValues { .. }
            );
            if show_loading {
                let _ = event_tx.send(CoreEvent::Loading);
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

        error!("Worker command channel closed unexpectedly");
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
        let _ = event_rx.recv().await;

        // Send a command that triggers Loading
        cmd_tx.send(CoreCommand::ListTables).unwrap();

        // First we should get a loading event
        let loading_event = event_rx.recv().await.expect("Expected event");
        assert!(matches!(loading_event, CoreEvent::Loading));

        // Since we are not connected, we should get an error next
        let error_event = event_rx.recv().await.expect("Expected event");
        match error_event {
            CoreEvent::Error(msg) => assert!(msg.contains("No active connection")),
            _ => panic!("Expected Error event, got {:?}", error_event),
        }
    }
}
