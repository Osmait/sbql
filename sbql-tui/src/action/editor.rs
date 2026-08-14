//! The SQL editor pane.

use super::*;

/// The SQL editor pane.
pub(super) fn apply(
    action: EditorAction,
    state: &mut AppState,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    match action {
        // -- Editor --
        EditorAction::Input(input) => {
            state.editor.textarea.input(input);
            state.editor.mark_text_changed();
            // Recompute completions inline
            completion_ui::recompute_completions(state);
        }

        EditorAction::CursorMove(mv) => {
            state.editor.textarea.move_cursor(mv);
        }

        EditorAction::RunQuery => {
            let sql = state.editor.sql();
            if !sql.trim().is_empty() {
                // The sort is not cleared here: core drops it for the new
                // query and reports that back as `SortChanged`.
                state.active_filter = None;
                state.editor.completion.dismiss();
                // Promoted to `source_sql` when its result arrives, so
                // edits/deletes know which table produced the rows on screen.
                state.results.sent_sql = Some(sql.clone());
                let _ = cmd_tx.send(CoreCommand::ExecuteQuery { sql });
                state.focused = FocusedPanel::Results;
            }
        }
    }
}
