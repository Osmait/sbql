//! Moving around and acting on the results grid.

use super::*;

/// Moving around and acting on the results grid.
pub(super) fn apply(
    action: ResultsAction,
    state: &mut AppState,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    match action {
        // -- Results navigation --
        ResultsAction::RowDown => {
            if state.results.move_row_down_with_page_hint() {
                let next = state.results.current_page + 1;
                let _ = cmd_tx.send(CoreCommand::FetchPage { page: next });
            }
        }

        ResultsAction::RowUp => {
            state.results.move_row_up();
        }

        ResultsAction::ColRight => {
            state.results.move_col_right();
        }

        ResultsAction::ColLeft => {
            state.results.move_col_left();
        }

        ResultsAction::RowFirst => {
            state.results.move_row_first();
        }

        ResultsAction::RowLast => {
            state.results.move_row_last();
        }

        ResultsAction::HalfPageDown => {
            if state.results.move_row_half_page_down() {
                let next = state.results.current_page + 1;
                let _ = cmd_tx.send(CoreCommand::FetchPage { page: next });
            }
        }

        ResultsAction::HalfPageUp => {
            state.results.move_row_half_page_up();
        }

        ResultsAction::ColFirst => {
            state.results.move_col_first();
        }

        ResultsAction::ColLast => {
            state.results.move_col_last();
        }

        ResultsAction::SetRow(row) => {
            if row < state.results.data.rows.len() {
                state.results.selected_row = row;
            }
        }

        ResultsAction::SetCol(col) => {
            let max = state.results.data.columns.len().saturating_sub(1);
            state.results.selected_col = col.min(max);
        }

        ResultsAction::MarkRowForDeletion => {
            let row_idx = state.results.selected_row;
            let sql = state.editor.sql();
            let (schema, table) = crate::handlers::results::extract_schema_table_from_sql(&sql)
                .unwrap_or_else(|| ("public".into(), "unknown".into()));
            state.mutation.pending_delete_row = Some(row_idx);
            let _ = cmd_tx.send(CoreCommand::GetPrimaryKeys { schema, table });
        }

        ResultsAction::CommitPending => {
            apply_commit_pending(state, cmd_tx);
        }

        ResultsAction::DiscardPendingOrEsc => {
            if !state.mutation.pending_edits.is_empty()
                || !state.mutation.pending_deletes.is_empty()
            {
                state.mutation.discard_pending();
                state.status_msg = Some("Staged changes discarded.".into());
            } else {
                state.focused = FocusedPanel::Editor;
            }
        }

        ResultsAction::ToggleSort => {
            if let Some(col) = state.results.selected_column_name().map(str::to_owned) {
                let (col, dir) = state.results.toggle_sort(&col);
                match dir {
                    Some(d) => {
                        let _ = cmd_tx.send(CoreCommand::ApplyOrder {
                            column: col,
                            direction: d,
                        });
                    }
                    None => {
                        let _ = cmd_tx.send(CoreCommand::ClearOrder);
                    }
                }
            }
        }
    }
}

pub(super) fn apply_commit_pending(
    state: &mut AppState,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    if state.mutation.pending_edits.is_empty() && state.mutation.pending_deletes.is_empty() {
        state.error_msg = Some("Nothing to commit — no staged edits or deletes.".into());
        return;
    }

    let edit_count = state.mutation.pending_edits.len();
    let delete_count = state.mutation.pending_deletes.len();

    for edit in state.mutation.pending_edits.values() {
        let _ = cmd_tx.send(CoreCommand::UpdateCell {
            schema: edit.schema.clone(),
            table: edit.table.clone(),
            pk_col: edit.pk_col.clone(),
            pk_val: edit.pk_val.clone(),
            target_col: edit.col_name.clone(),
            new_val: edit.new_val.clone(),
        });
    }

    for del in state.mutation.pending_deletes.values() {
        let _ = cmd_tx.send(CoreCommand::DeleteRow {
            schema: del.schema.clone(),
            table: del.table.clone(),
            pk_col: del.pk_col.clone(),
            pk_val: del.pk_val.clone(),
        });
    }

    state.mutation.pending_edits.clear();
    state.mutation.pending_deletes.clear();
    state.mutation.pending_d = false;

    let page = state.results.current_page;
    let _ = cmd_tx.send(CoreCommand::FetchPage { page });

    state.status_msg = Some(format!(
        "Committed: {} edit(s), {} delete(s).",
        edit_count, delete_count
    ));
}
