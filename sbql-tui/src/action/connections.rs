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
            state.conn.form = ConnectionForm::open_new();
        }

        ConnectionsAction::OpenEditForm => {
            if let Some(cfg) = state.conn.connections.get(state.conn.selected()).cloned() {
                state.conn.form = ConnectionForm::open_edit(&cfg);
            }
        }

        ConnectionsAction::InitDelete => {
            if let Some(cfg) = state.conn.connections.get(state.conn.selected()).cloned() {
                state.conn.pending_delete = Some((cfg.id, cfg.name.clone()));
                state.status_msg = Some(format!(
                    "Confirm delete connection '{}': y/Enter = confirm, n/Esc = cancel.",
                    cfg.name
                ));
                state.error_msg = None;
            }
        }

        ConnectionsAction::ConfirmDelete => {
            if let Some((id, name)) = state.conn.pending_delete.take() {
                let _ = cmd_tx.send(CoreCommand::DeleteConnection(id));
                state.status_msg = Some(format!("Deleted connection '{name}'."));
                state.error_msg = None;
            }
        }

        ConnectionsAction::CancelDelete => {
            state.conn.pending_delete = None;
            state.status_msg = Some("Delete cancelled.".into());
            state.error_msg = None;
        }

        ConnectionsAction::DisconnectActive => {
            if let Some(id) = state.conn.active_id {
                let _ = cmd_tx.send(CoreCommand::Disconnect(id));
            }
        }
    }
}
