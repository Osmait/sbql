//! The add/edit connection form.

use super::*;

/// The add/edit connection form.
pub(super) fn apply(
    action: FormAction,
    state: &mut AppState,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    match action {
        // -- Connection form --
        FormAction::Close => {
            state.conn.form.visible = false;
        }

        FormAction::FocusField(idx) => {
            // Clamped rather than ignored: the row list shrinks when the
            // backend changes, and a click mid-repaint must not wander.
            let count = state.conn.form.field_count();
            if count > 0 {
                state.conn.form.field_index = idx.min(count - 1);
            }
        }

        FormAction::NextField => {
            let count = state.conn.form.field_count();
            state.conn.form.field_index = (state.conn.form.field_index + 1) % count;
        }

        FormAction::PrevField => {
            let count = state.conn.form.field_count();
            state.conn.form.field_index = state
                .conn
                .form
                .field_index
                .checked_sub(1)
                .unwrap_or(count - 1);
        }

        FormAction::Input(c) => {
            if let Some(val) = state.conn.form.active_value_mut() {
                val.push(c);
            }
        }

        FormAction::Backspace => {
            if let Some(val) = state.conn.form.active_value_mut() {
                val.pop();
            }
        }

        FormAction::CycleBackend => {
            state.conn.form.cycle_backend();
        }

        FormAction::CycleSsl => {
            state.conn.form.cycle_ssl_mode();
        }

        FormAction::Submit => {
            apply_form_submit(state, cmd_tx);
        }
    }
}

fn apply_form_submit(state: &mut AppState, cmd_tx: &mpsc::UnboundedSender<CoreCommand>) {
    // Validation and construction both live in sbql-core, so the rules here are
    // exactly the ones the macOS app and the save handler enforce.
    let (config, password) = match state.conn.form.draft.build() {
        Ok(config) => (config, state.conn.form.draft.password_for_save()),
        Err(e) => {
            // Send the user back to the field that needs attention.
            if let Some(idx) = state.conn.form.row_of(e.field) {
                state.conn.form.field_index = idx;
            }
            state.conn.form.error = Some(e.message);
            return;
        }
    };

    // The TUI's connection form has no SSH password field — it can enable a
    // tunnel and pick a key, but the password itself is only ever *read* back
    // when opening one. `None` therefore means "leave whatever is stored
    // alone", which is the only honest answer a form that cannot ask has.
    send_command(
        cmd_tx,
        CoreCommand::SaveConnection {
            config,
            password,
            ssh_password: None,
        },
    );
    state.conn.form.visible = false;
    state.conn.form.error = None;
}
