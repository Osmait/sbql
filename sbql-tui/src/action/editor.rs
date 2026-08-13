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

        EditorAction::ClickAt { row, col } => {
            // A fresh click drops any selection: the next drag starts here.
            state.editor.textarea.cancel_selection();
            jump_to(state, row, col);
        }

        EditorAction::DragTo { row, col } => {
            // The selection anchor is set on the first drag event, not on the
            // click — starting it on mouse-down would leave a live selection
            // behind for every plain click.
            if !state.editor.dragging {
                state.editor.textarea.start_selection();
                state.editor.dragging = true;
            }
            jump_to(state, row, col);
        }

        EditorAction::DragEnd => {
            // The selection itself stays — only the "a drag is in progress"
            // latch is released, so the next drag anchors afresh.
            state.editor.dragging = false;
        }

        EditorAction::Scroll(delta) => {
            state.editor.textarea.scroll((delta, 0));
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

/// Move the cursor to a text position, clamped by tui-textarea itself.
///
/// `Jump` takes `u16`, and a click can only ever land on a screen cell, so a
/// position that does not fit is a position that was never on screen.
fn jump_to(state: &mut AppState, row: usize, col: usize) {
    let (Ok(row), Ok(col)) = (u16::try_from(row), u16::try_from(col)) else {
        return;
    };
    state
        .editor
        .textarea
        .move_cursor(tui_textarea::CursorMove::Jump(row, col));
}
