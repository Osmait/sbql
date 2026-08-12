//! The single-cell edit overlay.

use super::*;

/// The single-cell edit overlay.
pub(super) fn apply(
    action: CellEditAction,
    state: &mut AppState,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    match action {
        // -- Results actions --
        CellEditAction::Enter => {
            apply_enter_cell_edit(state, cmd_tx);
        }

        CellEditAction::Stage => {
            apply_stage_cell_edit(state);
        }

        CellEditAction::Cancel => {
            state.mutation.cell_edit = None;
        }

        CellEditAction::Input(input) => {
            if let Some(ce) = state.mutation.cell_edit.as_mut() {
                ce.textarea.input(input);
            }
        }
    }
}

fn apply_enter_cell_edit(state: &mut AppState, cmd_tx: &mpsc::UnboundedSender<CoreCommand>) {
    let row_idx = state.results.selected_row;
    let col_idx = state.results.selected_col;

    if state.results.data.columns.get(col_idx).is_none() {
        return;
    }
    if state.results.data.rows.get(row_idx).is_none() {
        return;
    }

    // Resolve the table from the query that produced the displayed rows, not
    // the editor text — an unexecuted query there redirected the UPDATE at a
    // different table.
    let Some((schema, table_name)) = super::results::source_table(state) else {
        state.report("Cannot edit: no table resolved from the executed query.");
        return;
    };
    tracing::info!(
        "enter_cell_edit_mode: schema={:?} table={:?}",
        schema,
        table_name
    );

    state.mutation.pending_cell_edit = Some((row_idx, col_idx));
    tracing::info!("GetPrimaryKeys: schema={:?} table={:?}", schema, table_name);
    let _ = cmd_tx.send(CoreCommand::GetPrimaryKeys {
        schema,
        table: table_name,
    });
}

pub(super) fn apply_stage_cell_edit(state: &mut AppState) {
    if let Some(ce) = state.mutation.cell_edit.take() {
        let new_val = ce.current_value();
        let col_name = ce.col_name.clone();
        if new_val != ce.original {
            state.mutation.pending_edits.insert(
                (ce.row_idx, ce.col_idx),
                PendingEdit {
                    new_val,
                    schema: ce.schema,
                    table: ce.table,
                    pk: ce.pk,
                    col_name: ce.col_name,
                },
            );
            let total = state.mutation.pending_edits.len() + state.mutation.pending_deletes.len();
            state.inform(format!(
                "Staged edit on '{}'. Total staged: {}. Press Ctrl+W to commit.",
                col_name, total
            ));
        } else {
            state.inform("No changes to stage (value unchanged).");
        }
    }
}
