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
            state.editor.invalidate_highlight();
            // Recompute completions inline
            completion_ui::recompute_completions(state);
        }

        EditorAction::CursorMove(mv) => {
            state.editor.textarea.move_cursor(mv);
        }

        EditorAction::RunQuery => {
            let sql = state.editor.sql();
            if !sql.trim().is_empty() {
                state.results.sort_state.clear();
                state.active_filter = None;
                state.editor.completion.dismiss();
                let _ = cmd_tx.send(CoreCommand::ExecuteQuery { sql });
                state.focused = FocusedPanel::Results;
            }
        }
    }
}
