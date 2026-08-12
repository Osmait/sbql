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
                if has_staged_changes(state) {
                    warn_staged_changes_block_paging(state);
                } else {
                    let next = state.results.current_page + 1;
                    let _ = cmd_tx.send(CoreCommand::FetchPage { page: next });
                }
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
                if has_staged_changes(state) {
                    warn_staged_changes_block_paging(state);
                } else {
                    let next = state.results.current_page + 1;
                    let _ = cmd_tx.send(CoreCommand::FetchPage { page: next });
                }
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
            // The table must come from the query that produced these rows —
            // parsing the live editor text let an unexecuted query redirect
            // the DELETE at a different table.
            let Some((schema, table)) = source_table(state) else {
                state.report("Cannot delete: no table resolved from the executed query.");
                return;
            };
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
                state.inform("Staged changes discarded.");
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

pub(super) fn has_staged_changes(state: &AppState) -> bool {
    !state.mutation.pending_edits.is_empty() || !state.mutation.pending_deletes.is_empty()
}

/// Paging replaces the rows the staged changes were made against, and the
/// arriving result discards them — so it is refused, loudly, instead.
pub(super) fn warn_staged_changes_block_paging(state: &mut AppState) {
    state
        .inform("Staged changes pending — commit (Ctrl+W) or discard (Esc) before changing pages.");
}

/// The `(schema, table)` the currently displayed rows came from.
pub(super) fn source_table(state: &AppState) -> Option<(String, String)> {
    let sql = state.results.source_sql.as_ref()?;
    crate::handlers::results::extract_schema_table_from_sql(sql)
}

pub(super) fn apply_commit_pending(
    state: &mut AppState,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    if state.mutation.pending_edits.is_empty() && state.mutation.pending_deletes.is_empty() {
        state.report("Nothing to commit — no staged edits or deletes.");
        return;
    }

    let edit_count = state.mutation.pending_edits.len();
    let delete_count = state.mutation.pending_deletes.len();

    for edit in state.mutation.pending_edits.values() {
        let _ = cmd_tx.send(CoreCommand::UpdateCell {
            schema: edit.schema.clone(),
            table: edit.table.clone(),
            pk: edit.pk.clone(),
            target_col: edit.col_name.clone(),
            new_val: edit.new_val.clone(),
        });
    }

    for del in state.mutation.pending_deletes.values() {
        let _ = cmd_tx.send(CoreCommand::DeleteRow {
            schema: del.schema.clone(),
            table: del.table.clone(),
            pk: del.pk.clone(),
        });
    }

    state.mutation.pending_edits.clear();
    state.mutation.pending_deletes.clear();
    state.mutation.pending_d = false;

    let page = state.results.current_page;
    let _ = cmd_tx.send(CoreCommand::FetchPage { page });

    state.inform(format!(
        "Committed: {} edit(s), {} delete(s).",
        edit_count, delete_count
    ));
}
