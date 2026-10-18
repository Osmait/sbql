//! The connection list — saved connections and this session's Docker finds.

use super::*;

/// The connection list — saved connections and this session's Docker finds.
pub(super) fn apply(
    action: ConnectionsAction,
    state: &mut AppState,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    match action {
        // -- Connections --
        ConnectionsAction::Select(idx) => {
            if !state.conn.is_empty() {
                state.conn.cursor.select(idx, state.conn.len());
            }
        }

        ConnectionsAction::ConnectSelected => {
            if let Some(entry) = state.conn.selected_entry() {
                let id = entry.config().id;
                send_command(cmd_tx, CoreCommand::Connect(id));
            }
        }

        ConnectionsAction::OpenNewForm => {
            state.close_overlays();
            state.conn.form = ConnectionForm::open_new();
        }

        ConnectionsAction::OpenEditForm => {
            // Editing a discovered connection is offered as "save it first":
            // there is nothing on disk to edit, and silently turning the edit
            // into a new saved connection would be a surprising way to persist
            // credentials the user only wanted for this session.
            match state.conn.selected_entry() {
                Some(ConnectionEntry::Saved(cfg)) => {
                    let cfg = cfg.clone();
                    state.close_overlays();
                    state.conn.form = ConnectionForm::open_edit(&cfg);
                }
                Some(ConnectionEntry::Discovered(_)) => {
                    state.report("Docker connection — press s to save it first, then edit.");
                }
                None => {}
            }
        }

        ConnectionsAction::SaveDiscovered => {
            match state.conn.selected_entry() {
                Some(ConnectionEntry::Discovered(found)) => {
                    let (id, name) = (found.config.id, found.config.name.clone());
                    send_command(cmd_tx, CoreCommand::SaveDiscovered(id));
                    state.inform(format!("Saved '{name}' — it will be there next launch."));
                }
                // Saying nothing on a saved row would look like the key is
                // broken; it is simply not what `s` is for.
                Some(ConnectionEntry::Saved(_)) => {
                    state.inform("Already saved.");
                }
                None => {}
            }
        }

        ConnectionsAction::InitDelete => {
            match state.conn.selected_entry() {
                Some(ConnectionEntry::Saved(cfg)) => {
                    let pending = (cfg.id, cfg.name.clone());
                    let name = cfg.name.clone();
                    state.close_overlays();
                    state.conn.pending_delete = Some(pending);
                    state.inform(format!(
                        "Confirm delete connection '{name}': y/Enter = confirm, n/Esc = cancel."
                    ));
                }
                // Nothing to delete: it was never written anywhere. It goes
                // away on its own when the container stops.
                Some(ConnectionEntry::Discovered(_)) => {
                    state.report("Docker connection — nothing saved to delete.");
                }
                None => {}
            }
        }

        ConnectionsAction::ConfirmDelete => {
            if let Some((id, name)) = state.conn.pending_delete.take() {
                send_command(cmd_tx, CoreCommand::DeleteConnection(id));
                state.inform(format!("Deleted connection '{name}'."));
            }
        }

        ConnectionsAction::CancelDelete => {
            state.conn.pending_delete = None;
            state.inform("Delete cancelled.");
        }

        ConnectionsAction::DisconnectActive => {
            if let Some(id) = state.conn.active_id {
                send_command(cmd_tx, CoreCommand::Disconnect(id));
            }
        }
    }
}
