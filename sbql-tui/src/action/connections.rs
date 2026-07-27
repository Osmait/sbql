//! The saved-connection list.

use super::*;

/// The saved-connection list.
pub(super) fn apply(
    action: ConnectionsAction,
    state: &mut AppState,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    match action {
        // -- Connections --
        ConnectionsAction::Select(idx) => {
            if !state.conn.connections.is_empty() {
                state.conn.cursor.select(idx, state.conn.connections.len());
            }
        }

        ConnectionsAction::ConnectSelected => {
            if let Some(cfg) = state.conn.connections.get(state.conn.selected()) {
                let id = cfg.id;
                let _ = cmd_tx.send(CoreCommand::Connect(id));
            }
        }

        ConnectionsAction::OpenNewForm => {
            state.close_overlays();
            state.conn.form = ConnectionForm::open_new();
        }

        ConnectionsAction::OpenEditForm => {
            if let Some(cfg) = state.conn.connections.get(state.conn.selected()).cloned() {
                state.close_overlays();
                state.conn.form = ConnectionForm::open_edit(&cfg);
            }
        }

        ConnectionsAction::InitDelete => {
            if let Some(cfg) = state.conn.connections.get(state.conn.selected()).cloned() {
                let pending = (cfg.id, cfg.name.clone());
                state.close_overlays();
                state.conn.pending_delete = Some(pending);
                state.inform(format!(
                    "Confirm delete connection '{}': y/Enter = confirm, n/Esc = cancel.",
                    cfg.name
                ));
            }
        }

        ConnectionsAction::ConfirmDelete => {
            if let Some((id, name)) = state.conn.pending_delete.take() {
                let _ = cmd_tx.send(CoreCommand::DeleteConnection(id));
                state.inform(format!("Deleted connection '{name}'."));
            }
        }

        ConnectionsAction::CancelDelete => {
            state.conn.pending_delete = None;
            state.inform("Delete cancelled.");
        }

        ConnectionsAction::DisconnectActive => {
            if let Some(id) = state.conn.active_id {
                let _ = cmd_tx.send(CoreCommand::Disconnect(id));
            }
        }
    }
}
