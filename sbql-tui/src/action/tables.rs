//! The table browser.

use super::*;

/// The table browser.
pub(super) fn apply(
    action: TablesAction,
    state: &mut AppState,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    match action {
        // -- Tables --
        TablesAction::Select(idx) => {
            state.tables.cursor.select(idx, state.tables.tables.len());
        }

        TablesAction::OpenSelected => {
            if let Some(t) = state.tables.tables.get(state.tables.selected()) {
                let sql = sbql_core::query_builder::table_select_sql(
                    &t.schema,
                    &t.name,
                    state.conn.active_backend,
                );
                // `table_select_sql` returns an empty string for the non-SQL
                // backends (Redis/DynamoDB/MongoDB); sending that as a query is
                // a guaranteed error, so tell the user the action isn't
                // available here instead.
                if sql.trim().is_empty() {
                    state.report("Opening a table this way isn't supported for this backend.");
                    return;
                }
                tracing::info!(
                    "open_selected_table: schema={:?} table={:?} sql={:?}",
                    t.schema,
                    t.name,
                    sql
                );
                // The sort is not cleared here: core drops it for the new
                // query and reports that back as `SortChanged`.
                state.active_filter = None;
                state.editor.textarea = {
                    let mut ta = tui_textarea::TextArea::default();
                    ta.set_placeholder_text("-- Write SQL here. Press Ctrl+S or F5 to run.");
                    ta.insert_str(&sql);
                    ta
                };
                state.results.sent_sql = Some(sql.clone());
                let _ = cmd_tx.send(CoreCommand::ExecuteQuery { sql });
                state.focused = FocusedPanel::Results;
            }
        }
    }
}
