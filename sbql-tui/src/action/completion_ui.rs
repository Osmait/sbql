//! The autocomplete popup.

use super::*;

/// The autocomplete popup.
pub(super) fn apply(
    action: CompletionAction,
    state: &mut AppState,
    _cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    match action {
        // -- Completion --
        CompletionAction::Up => {
            state.editor.completion.move_up();
        }

        CompletionAction::Down => {
            state.editor.completion.move_down();
        }

        CompletionAction::Accept => {
            if let Some(item) = state.editor.completion.selected_item().cloned() {
                // One backspace deletes one *character*, so count chars — with
                // byte len a multi-byte prefix ate extra characters before it.
                let prefix_len = state.editor.completion.prefix.chars().count();
                // Delete the prefix by sending backspace inputs
                for _ in 0..prefix_len {
                    state.editor.textarea.input(Input {
                        key: tui_textarea::Key::Backspace,
                        ctrl: false,
                        alt: false,
                        shift: false,
                    });
                }
                // Insert the completion text char-by-char
                for ch in item.text.chars() {
                    state.editor.textarea.input(Input {
                        key: tui_textarea::Key::Char(ch),
                        ctrl: false,
                        alt: false,
                        shift: false,
                    });
                }
                state.editor.mark_text_changed();
                state.editor.completion.dismiss();
            }
        }

        CompletionAction::Dismiss => {
            state.editor.completion.dismiss();
        }
    }
}

/// Recompute autocomplete completions based on current editor state.
pub(super) fn recompute_completions(state: &mut AppState) {
    let lines: Vec<String> = state
        .editor
        .textarea
        .lines()
        .iter()
        .map(|s| s.to_string())
        .collect();
    let (row, col) = state.editor.textarea.cursor();
    let prefix = completion::extract_prefix(&lines, row, col);
    if prefix.len() >= 2 {
        let items = completion::compute_completions(
            &prefix,
            &state.tables.tables,
            state.cached_diagram.as_ref(),
        );
        if items.is_empty() {
            state.editor.completion.dismiss();
        } else {
            state.editor.completion.prefix = prefix;
            state.editor.completion.items = items;
            state.editor.completion.cursor.reset();
            state.editor.completion.visible = true;
        }
    } else {
        state.editor.completion.dismiss();
    }
}
