//! Focus, editor/nav modes, and pending vim prefixes.

use super::*;

/// Focus, editor/nav modes, and pending vim prefixes.
pub(super) fn apply(
    action: NavAction,
    state: &mut AppState,
    _cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    match action {
        // -- Navigation --
        NavAction::FocusPanel(p) => {
            state.focused = if state.layout.sidebar_hidden
                && (p == FocusedPanel::Connections || p == FocusedPanel::Tables)
            {
                FocusedPanel::Editor
            } else {
                p
            };
        }

        NavAction::SetNavMode(m) => {
            state.vim.nav_mode = m;
        }

        NavAction::SetEditorMode(m) => {
            state.editor.mode = m;
            if m == EditorMode::Normal {
                state.editor.completion.dismiss();
            }
        }

        NavAction::ToggleSidebar => {
            state.layout.sidebar_hidden = !state.layout.sidebar_hidden;
            if state.layout.sidebar_hidden
                && (state.focused == FocusedPanel::Connections
                    || state.focused == FocusedPanel::Tables)
            {
                state.focused = FocusedPanel::Editor;
            }
            state.inform(if state.layout.sidebar_hidden {
                "Sidebar hidden"
            } else {
                "Sidebar shown"
            });
        }

        // -- Vim state --
        NavAction::ClearPendingG => {
            state.vim.pending_g = false;
        }

        NavAction::SetPendingG => {
            state.vim.pending_g = true;
        }

        NavAction::ClearPendingD => {
            state.mutation.pending_d = false;
        }

        NavAction::SetPendingD => {
            state.mutation.pending_d = true;
        }

        NavAction::SetPendingLeader(v) => {
            state.vim.pending_leader = v;
        }
    }
}
