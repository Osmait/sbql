//! The results filter bar.

use super::*;

/// The results filter bar.
pub(super) fn apply(
    action: FilterAction,
    state: &mut AppState,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    match action {
        FilterAction::Open => {
            state.filter.visible = true;
            state.filter.textarea = tui_textarea::TextArea::default();
            state.filter.suggestions.clear();
            state.filter.suggestion_cursor.reset();
            state.filter.show_suggestions = false;
            state.filter.loading_suggestions = false;
            state.filter.pending_live_apply_at = None;
            state.filter.last_applied_query = state.active_filter.clone();
        }

        // -- Filter --
        FilterAction::CloseSuggestions => {
            state.filter.show_suggestions = false;
            state.filter.loading_suggestions = false;
        }

        FilterAction::Close => {
            state.filter.visible = false;
            state.filter.show_suggestions = false;
            state.filter.loading_suggestions = false;
            state.filter.pending_live_apply_at = None;
            state.filter.last_applied_query = None;
            state.active_filter = None;
            let _ = cmd_tx.send(CoreCommand::ClearFilter);
        }

        FilterAction::Input(input) => {
            state.filter.textarea.input(input);
            apply_refresh_filter_suggestions(state, cmd_tx);
        }

        FilterAction::SuggestionUp => {
            state
                .filter
                .suggestion_cursor
                .prev(state.filter.suggestions.len(), Overflow::Clamp);
        }

        FilterAction::SuggestionDown => {
            state
                .filter
                .suggestion_cursor
                .next(state.filter.suggestions.len(), Overflow::Clamp);
        }

        FilterAction::ApplySuggestion => {
            if apply_selected_filter_suggestion(state) {
                apply_refresh_filter_suggestions(state, cmd_tx);
            }
        }

        FilterAction::Apply => {
            let query = state.filter.textarea.lines().join("");
            state.filter.visible = false;
            state.filter.show_suggestions = false;
            state.filter.loading_suggestions = false;
            state.filter.pending_live_apply_at = None;
            if query.trim().is_empty() {
                state.active_filter = None;
                state.filter.last_applied_query = None;
                let _ = cmd_tx.send(CoreCommand::ClearFilter);
            } else {
                state.active_filter = Some(query.clone());
                state.filter.last_applied_query = Some(query.clone());
                let _ = cmd_tx.send(CoreCommand::ApplyFilter { query });
            }
        }
    }
}

pub(super) fn apply_refresh_filter_suggestions(
    state: &mut AppState,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    let input = state.filter.textarea.lines().join("");
    let trimmed = input.trim();

    if trimmed.is_empty() {
        state.filter.suggestions.clear();
        state.filter.show_suggestions = false;
        state.filter.loading_suggestions = false;
        state.filter.pending_live_apply_at = None;
        return;
    }

    if !trimmed.contains(':') {
        let prefix = trimmed.to_lowercase();
        let mut suggestions: Vec<String> = state
            .results
            .data
            .columns
            .iter()
            .filter(|c| c.to_lowercase().starts_with(&prefix))
            .take(20)
            .cloned()
            .collect();
        suggestions.sort();
        state.filter.suggestions = suggestions;
        state.filter.suggestion_cursor.reset();
        state.filter.show_suggestions = !state.filter.suggestions.is_empty();
        state.filter.loading_suggestions = false;
        state.filter.pending_live_apply_at = None;
        return;
    }

    let Some((col_raw, value_prefix)) = parse_filter_input(trimmed) else {
        state.filter.suggestions.clear();
        state.filter.show_suggestions = false;
        state.filter.loading_suggestions = false;
        state.filter.pending_live_apply_at = None;
        return;
    };

    let Some(col) = state
        .results
        .data
        .columns
        .iter()
        .find(|c| c.eq_ignore_ascii_case(&col_raw))
        .cloned()
    else {
        state.filter.suggestions.clear();
        state.filter.show_suggestions = false;
        state.filter.loading_suggestions = false;
        state.filter.pending_live_apply_at = None;
        return;
    };

    let col_idx = match state
        .results
        .data
        .columns
        .iter()
        .position(|c| c.eq_ignore_ascii_case(&col))
    {
        Some(i) => i,
        None => return,
    };
    let prefix_lower = value_prefix.to_lowercase();
    let mut local = std::collections::BTreeSet::new();
    for row in &state.results.data.rows {
        if let Some(v) = row.get(col_idx) {
            if v.to_lowercase().starts_with(&prefix_lower) {
                local.insert(v.clone());
            }
        }
        if local.len() >= 20 {
            break;
        }
    }
    state.filter.suggestions = local.into_iter().collect();
    state.filter.suggestion_cursor.reset();
    state.filter.show_suggestions = true;

    state.filter.suggestion_token = state.filter.suggestion_token.saturating_add(1);
    state.filter.loading_suggestions = true;
    state.filter.pending_live_apply_at =
        Some(std::time::Instant::now() + std::time::Duration::from_millis(250));
    let _ = cmd_tx.send(CoreCommand::SuggestFilterValues {
        column: col,
        prefix: value_prefix.to_owned(),
        limit: 20,
        token: state.filter.suggestion_token,
    });
}

pub(super) fn parse_filter_input(input: &str) -> Option<(String, &str)> {
    let colon = input.find(':')?;
    let col = input[..colon].trim();
    if col.is_empty() {
        return None;
    }
    let value = input[colon + 1..].trim_start();
    Some((col.to_owned(), value))
}

fn apply_selected_filter_suggestion(state: &mut AppState) -> bool {
    if !state.filter.show_suggestions || state.filter.suggestions.is_empty() {
        return false;
    }
    let Some(choice) = state
        .filter
        .suggestions
        .get(state.filter.suggestion_cursor.index())
        .cloned()
    else {
        return false;
    };

    let current = state.filter.textarea.lines().join("");
    let replacement = if let Some(colon) = current.find(':') {
        let col = current[..colon].trim();
        format!("{col}:{choice}")
    } else {
        format!("{choice}:")
    };

    if replacement == current {
        return false;
    }

    state.filter.textarea = tui_textarea::TextArea::default();
    state.filter.textarea.insert_str(&replacement);
    true
}
