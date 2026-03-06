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
