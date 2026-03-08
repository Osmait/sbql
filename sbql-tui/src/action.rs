use tokio::sync::mpsc;
use tui_textarea::{CursorMove, Input};

use crate::app::{
    AppState, ConnectionForm, DiagramGlyphMode, EditorMode, FocusedPanel,
    NavMode, PendingEdit,
};
use crate::completion;
use sbql_core::CoreCommand;

/// A pure description of a state change or side effect.
///
/// Handlers produce `Action` values without mutating state directly.
/// The event loop calls [`apply`] to execute them.
#[derive(Debug)]
pub enum Action {
    // -- Navigation / focus --
    FocusPanel(FocusedPanel),
    SetNavMode(NavMode),
    SetEditorMode(EditorMode),
    ToggleSidebar,

    // -- Results grid navigation --
    MoveRowDown,
    MoveRowUp,
    MoveColRight,
    MoveColLeft,
    MoveRowFirst,
    MoveRowLast,
    MoveHalfPageDown,
    MoveHalfPageUp,
    MoveColFirst,
    MoveColLast,
    SetResultsRow(usize),
    SetResultsCol(usize),

    // -- Results actions --
    EnterCellEdit,
    StageCellEdit,
    CancelCellEdit,
    CellEditInput(Input),
    MarkRowForDeletion,
    CommitPending,
    DiscardPendingOrEsc,
    ToggleSort,
    OpenFilter,

    // -- Editor --
    EditorInput(Input),
    EditorCursorMove(CursorMove),
    RunQuery,

    // -- Completion --
    CompletionUp,
    CompletionDown,
    CompletionAccept,
    CompletionDismiss,

    // -- Connections --
    ConnectSelected,
    OpenNewConnForm,
    OpenEditConnForm,
    InitDeleteConnection,
    ConfirmDeleteConnection,
    CancelDeleteConnection,
    DisconnectActive,

    // -- Connection form --
    FormClose,
    FormNextField,
    FormPrevField,
    FormInput(char),
    FormBackspace,
    FormCycleBackend,
    FormCycleSsl,
    FormSubmit,

    // -- Connections --
    SelectConnection(usize),

    // -- Tables --
    SelectTable(usize),
    OpenSelectedTable,

    // -- Filter --
    FilterCloseSuggestions,
    FilterClose,
    FilterInput(Input),
    FilterSuggestionUp,
    FilterSuggestionDown,
    FilterApplySuggestion,
    FilterApply,

    // -- Diagram --
    OpenDiagram,
    CloseDiagram,
    DiagramScroll { dx: i16, dy: i16 },
    DiagramSelectNext,
    DiagramSelectPrev,
    DiagramSelectFirst,
    DiagramSelectLast,
    DiagramToggleFocus,
    DiagramToggleGlyph,
    DiagramJumpToTable,
    DiagramSearchOpen,
    DiagramSearchClose,
    DiagramSearchInput(char),
    DiagramSearchBackspace,
    DiagramSearchConfirm,

    // -- Vim state --
    ClearPendingG,
    SetPendingG,
    ClearPendingD,
    SetPendingD,
    SetPendingLeader(bool),

    // -- Status --
    SetStatus(Option<String>),
    SetError(Option<String>),
    Quit,
    Noop,

    // -- Side effects --
    SendCommand(CoreCommand),
    Batch(Vec<Action>),
}

/// Apply an action to state and send any commands.
pub fn apply(action: Action, state: &mut AppState, cmd_tx: &mpsc::UnboundedSender<CoreCommand>) {
    match action {
        // -- Navigation --
        Action::FocusPanel(p) => {
            state.focused = if state.layout.sidebar_hidden
                && (p == FocusedPanel::Connections || p == FocusedPanel::Tables)
            {
                FocusedPanel::Editor
            } else {
                p
            };
        }
        Action::SetNavMode(m) => {
            state.vim.nav_mode = m;
        }
        Action::SetEditorMode(m) => {
            state.editor.mode = m;
            if m == EditorMode::Normal {
                state.editor.completion.dismiss();
            }
        }
        Action::ToggleSidebar => {
            state.layout.sidebar_hidden = !state.layout.sidebar_hidden;
            if state.layout.sidebar_hidden
                && (state.focused == FocusedPanel::Connections
                    || state.focused == FocusedPanel::Tables)
            {
                state.focused = FocusedPanel::Editor;
            }
            state.status_msg = Some(if state.layout.sidebar_hidden {
                "Sidebar hidden".into()
            } else {
                "Sidebar shown".into()
            });
            state.error_msg = None;
        }

        // -- Results navigation --
        Action::MoveRowDown => {
            if state.results.move_row_down_with_page_hint() {
                let next = state.results.current_page + 1;
                let _ = cmd_tx.send(CoreCommand::FetchPage { page: next });
            }
        }
        Action::MoveRowUp => {
            state.results.move_row_up();
        }
        Action::MoveColRight => {
            state.results.move_col_right();
        }
        Action::MoveColLeft => {
            state.results.move_col_left();
        }
        Action::MoveRowFirst => {
            state.results.move_row_first();
        }
        Action::MoveRowLast => {
            state.results.move_row_last();
        }
        Action::MoveHalfPageDown => {
            if state.results.move_row_half_page_down() {
                let next = state.results.current_page + 1;
                let _ = cmd_tx.send(CoreCommand::FetchPage { page: next });
            }
        }
        Action::MoveHalfPageUp => {
            state.results.move_row_half_page_up();
        }
        Action::MoveColFirst => {
            state.results.move_col_first();
        }
        Action::MoveColLast => {
            state.results.move_col_last();
        }
        Action::SetResultsRow(row) => {
            if row < state.results.data.rows.len() {
                state.results.selected_row = row;
            }
        }
        Action::SetResultsCol(col) => {
            let max = state.results.data.columns.len().saturating_sub(1);
            state.results.selected_col = col.min(max);
        }

        // -- Results actions --
        Action::EnterCellEdit => {
            apply_enter_cell_edit(state, cmd_tx);
        }
        Action::StageCellEdit => {
            apply_stage_cell_edit(state);
        }
        Action::CancelCellEdit => {
            state.mutation.cell_edit = None;
        }
        Action::CellEditInput(input) => {
            if let Some(ce) = state.mutation.cell_edit.as_mut() {
                ce.textarea.input(input);
            }
        }
        Action::MarkRowForDeletion => {
            let row_idx = state.results.selected_row;
            let sql = state.editor.sql();
            let (schema, table) =
                crate::handlers::results::extract_schema_table_from_sql(&sql)
                    .unwrap_or_else(|| ("public".into(), "unknown".into()));
            state.mutation.pending_delete_row = Some(row_idx);
            let _ = cmd_tx.send(CoreCommand::GetPrimaryKeys { schema, table });
        }
        Action::CommitPending => {
            apply_commit_pending(state, cmd_tx);
        }
        Action::DiscardPendingOrEsc => {
            if !state.mutation.pending_edits.is_empty()
                || !state.mutation.pending_deletes.is_empty()
            {
                state.mutation.discard_pending();
                state.status_msg = Some("Staged changes discarded.".into());
            } else {
                state.focused = FocusedPanel::Editor;
            }
        }
        Action::ToggleSort => {
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
        Action::OpenFilter => {
            state.filter.visible = true;
            state.filter.textarea = tui_textarea::TextArea::default();
            state.filter.suggestions.clear();
            state.filter.selected_suggestion = 0;
            state.filter.show_suggestions = false;
            state.filter.loading_suggestions = false;
            state.filter.pending_live_apply_at = None;
            state.filter.last_applied_query = state.active_filter.clone();
        }

        // -- Editor --
        Action::EditorInput(input) => {
            state.editor.textarea.input(input);
            state.editor.invalidate_highlight();
            // Recompute completions inline
            recompute_completions(state);
        }
        Action::EditorCursorMove(mv) => {
            state.editor.textarea.move_cursor(mv);
        }
        Action::RunQuery => {
            let sql = state.editor.sql();
            if !sql.trim().is_empty() {
                state.results.sort_state.clear();
                state.active_filter = None;
                state.editor.completion.dismiss();
                let _ = cmd_tx.send(CoreCommand::ExecuteQuery { sql });
                state.focused = FocusedPanel::Results;
            }
        }

        // -- Completion --
        Action::CompletionUp => {
            state.editor.completion.move_up();
        }
        Action::CompletionDown => {
            state.editor.completion.move_down();
        }
        Action::CompletionAccept => {
            if let Some(item) = state.editor.completion.selected_item().cloned() {
                let prefix_len = state.editor.completion.prefix.len();
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
                state.editor.invalidate_highlight();
                state.editor.completion.dismiss();
            }
        }
        Action::CompletionDismiss => {
            state.editor.completion.dismiss();
        }

        // -- Connections --
        Action::SelectConnection(idx) => {
            if !state.conn.connections.is_empty() {
                state.conn.selected = idx.min(state.conn.connections.len() - 1);
            }
        }
        Action::ConnectSelected => {
            if let Some(cfg) = state.conn.connections.get(state.conn.selected) {
                let id = cfg.id;
                let _ = cmd_tx.send(CoreCommand::Connect(id));
            }
        }
        Action::OpenNewConnForm => {
            state.conn.form = ConnectionForm::open_new();
        }
        Action::OpenEditConnForm => {
            if let Some(cfg) = state.conn.connections.get(state.conn.selected).cloned() {
                state.conn.form = ConnectionForm::open_edit(&cfg);
            }
        }
        Action::InitDeleteConnection => {
            if let Some(cfg) = state.conn.connections.get(state.conn.selected).cloned() {
                state.conn.pending_delete = Some((cfg.id, cfg.name.clone()));
                state.status_msg = Some(format!(
                    "Confirm delete connection '{}': y/Enter = confirm, n/Esc = cancel.",
                    cfg.name
                ));
                state.error_msg = None;
            }
        }
        Action::ConfirmDeleteConnection => {
            if let Some((id, name)) = state.conn.pending_delete.take() {
                let _ = cmd_tx.send(CoreCommand::DeleteConnection(id));
                state.status_msg = Some(format!("Deleted connection '{name}'."));
                state.error_msg = None;
            }
        }
        Action::CancelDeleteConnection => {
            state.conn.pending_delete = None;
            state.status_msg = Some("Delete cancelled.".into());
            state.error_msg = None;
        }
        Action::DisconnectActive => {
            if let Some(id) = state.conn.active_id {
                let _ = cmd_tx.send(CoreCommand::Disconnect(id));
            }
        }

        // -- Connection form --
        Action::FormClose => {
            state.conn.form.visible = false;
        }
        Action::FormNextField => {
            let count = state.conn.form.field_count();
            state.conn.form.field_index = (state.conn.form.field_index + 1) % count;
        }
        Action::FormPrevField => {
            let count = state.conn.form.field_count();
            state.conn.form.field_index = state
                .conn
                .form
                .field_index
                .checked_sub(1)
                .unwrap_or(count - 1);
        }
        Action::FormInput(c) => {
            if let Some(val) = state.conn.form.active_value_mut() {
                val.push(c);
            }
        }
        Action::FormBackspace => {
            if let Some(val) = state.conn.form.active_value_mut() {
                val.pop();
            }
        }
        Action::FormCycleBackend => {
            state.conn.form.cycle_backend();
        }
        Action::FormCycleSsl => {
            state.conn.form.cycle_ssl_mode();
        }
        Action::FormSubmit => {
            apply_form_submit(state, cmd_tx);
        }

        // -- Tables --
        Action::SelectTable(idx) => {
            state.tables.selected = idx;
        }
        Action::OpenSelectedTable => {
            if let Some(t) = state.tables.tables.get(state.tables.selected) {
                let sql = sbql_core::query_builder::table_select_sql(&t.schema, &t.name, state.conn.active_backend);
                tracing::info!(
                    "open_selected_table: schema={:?} table={:?} sql={:?}",
                    t.schema,
                    t.name,
                    sql
                );
                state.results.sort_state.clear();
                state.active_filter = None;
                state.editor.textarea = {
                    let mut ta = tui_textarea::TextArea::default();
                    ta.set_placeholder_text("-- Write SQL here. Press Ctrl+S or F5 to run.");
                    ta.insert_str(&sql);
                    ta
                };
                let _ = cmd_tx.send(CoreCommand::ExecuteQuery { sql });
                state.focused = FocusedPanel::Results;
            }
        }

        // -- Filter --
        Action::FilterCloseSuggestions => {
            state.filter.show_suggestions = false;
            state.filter.loading_suggestions = false;
        }
        Action::FilterClose => {
            state.filter.visible = false;
            state.filter.show_suggestions = false;
            state.filter.loading_suggestions = false;
            state.filter.pending_live_apply_at = None;
            state.filter.last_applied_query = None;
            state.active_filter = None;
            let _ = cmd_tx.send(CoreCommand::ClearFilter);
        }
        Action::FilterInput(input) => {
            state.filter.textarea.input(input);
            apply_refresh_filter_suggestions(state, cmd_tx);
        }
        Action::FilterSuggestionUp => {
            state.filter.selected_suggestion =
                state.filter.selected_suggestion.saturating_sub(1);
        }
        Action::FilterSuggestionDown => {
            let max = state.filter.suggestions.len().saturating_sub(1);
            state.filter.selected_suggestion =
                (state.filter.selected_suggestion + 1).min(max);
        }
        Action::FilterApplySuggestion => {
            if apply_selected_filter_suggestion(state) {
                apply_refresh_filter_suggestions(state, cmd_tx);
            }
        }
        Action::FilterApply => {
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

        // -- Diagram --
        Action::OpenDiagram => {
            if state.conn.active_id.is_some() {
                state.diagram_requested = true;
                let _ = cmd_tx.send(CoreCommand::LoadDiagram);
            } else {
                state.error_msg =
                    Some("Connect to a database first (Enter on a connection).".into());
            }
        }
        Action::CloseDiagram => {
            state.diagram = None;
        }
        Action::DiagramScroll { dx, dy } => {
            if let Some(ref mut diag) = state.diagram {
                if dx > 0 {
                    diag.scroll_x = diag.scroll_x.saturating_add(dx as u16);
                } else {
                    diag.scroll_x = diag.scroll_x.saturating_sub((-dx) as u16);
                }
                if dy > 0 {
                    diag.scroll_y = diag.scroll_y.saturating_add(dy as u16);
                } else {
                    diag.scroll_y = diag.scroll_y.saturating_sub((-dy) as u16);
                }
            }
        }
        Action::DiagramSelectNext => {
            if let Some(ref mut diag) = state.diagram {
                let visible = diagram_visible_table_indices(diag);
                if !visible.is_empty() {
                    let pos = visible
                        .iter()
                        .position(|&i| i == diag.selected_table)
                        .unwrap_or(0);
                    let next = (pos + 1).min(visible.len() - 1);
                    diag.selected_table = visible[next];
                    diag.canvas_dirty = true;
                    diagram_keep_in_view(diag);
                }
            }
        }
        Action::DiagramSelectPrev => {
            if let Some(ref mut diag) = state.diagram {
                let visible = diagram_visible_table_indices(diag);
                if !visible.is_empty() {
                    let pos = visible
                        .iter()
                        .position(|&i| i == diag.selected_table)
                        .unwrap_or(0);
                    diag.selected_table = visible[pos.saturating_sub(1)];
                    diag.canvas_dirty = true;
                    diagram_keep_in_view(diag);
                }
            }
        }
        Action::DiagramSelectFirst => {
            if let Some(ref mut diag) = state.diagram {
                let visible = diagram_visible_table_indices(diag);
                if !visible.is_empty() {
                    diag.selected_table = visible[0];
                }
                diag.scroll_y = 0;
                diag.canvas_dirty = true;
            }
        }
        Action::DiagramSelectLast => {
            if let Some(ref mut diag) = state.diagram {
                let visible = diagram_visible_table_indices(diag);
                if !visible.is_empty() {
                    diag.selected_table = visible[visible.len() - 1];
                }
                diag.canvas_dirty = true;
            }
        }
        Action::DiagramToggleFocus => {
            if let Some(ref mut diag) = state.diagram {
                diag.focus_mode = !diag.focus_mode;
                diag.canvas_dirty = true;
                if diag.focus_mode {
                    let vis = diagram_visible_table_indices(diag);
                    if !vis.is_empty() && !vis.contains(&diag.selected_table) {
                        diag.selected_table = vis[0];
                    }
                }
            }
        }
        Action::DiagramToggleGlyph => {
            if let Some(ref mut diag) = state.diagram {
                diag.glyph_mode = match diag.glyph_mode {
                    DiagramGlyphMode::Ascii => DiagramGlyphMode::Unicode,
                    DiagramGlyphMode::Unicode => DiagramGlyphMode::Ascii,
                };
                diag.canvas_dirty = true;
            }
        }
        Action::DiagramJumpToTable => {
            if let Some(ref mut diag) = state.diagram {
                if let Some(&(tx, ty)) = diag.table_positions.get(&diag.selected_table) {
                    let vw = diag.last_viewport_w as usize;
                    let vh = diag.last_viewport_h as usize;
                    diag.scroll_x = (tx.saturating_sub(vw / 2)) as u16;
                    diag.scroll_y = (ty.saturating_sub(vh / 2)) as u16;
                }
            }
        }
        Action::DiagramSearchOpen => {
            if let Some(ref mut diag) = state.diagram {
                diag.search_active = true;
                diag.search_query.clear();
            }
        }
        Action::DiagramSearchClose => {
            if let Some(ref mut diag) = state.diagram {
                diag.search_active = false;
                diag.search_query.clear();
            }
        }
        Action::DiagramSearchInput(c) => {
            if let Some(ref mut diag) = state.diagram {
                diag.search_query.push(c);
                // Auto-select first matching table
                let query = diag.search_query.to_ascii_lowercase();
                let visible = diagram_visible_table_indices(diag);
                if let Some(&idx) = visible.iter().find(|&&i| {
                    diag.data
                        .tables
                        .get(i)
                        .map(|t| t.qualified().to_ascii_lowercase().contains(&query))
                        .unwrap_or(false)
                }) {
                    diag.selected_table = idx;
                    diag.canvas_dirty = true;
                }
            }
        }
        Action::DiagramSearchBackspace => {
            if let Some(ref mut diag) = state.diagram {
                diag.search_query.pop();
                if !diag.search_query.is_empty() {
                    let query = diag.search_query.to_ascii_lowercase();
                    let visible = diagram_visible_table_indices(diag);
                    if let Some(&idx) = visible.iter().find(|&&i| {
                        diag.data
                            .tables
                            .get(i)
                            .map(|t| t.qualified().to_ascii_lowercase().contains(&query))
                            .unwrap_or(false)
                    }) {
                        diag.selected_table = idx;
                        diag.canvas_dirty = true;
                    }
                }
            }
        }
        Action::DiagramSearchConfirm => {
            if let Some(ref mut diag) = state.diagram {
                diag.search_active = false;
                // Jump to the selected table
                if let Some(&(tx, ty)) = diag.table_positions.get(&diag.selected_table) {
                    let vw = diag.last_viewport_w as usize;
                    let vh = diag.last_viewport_h as usize;
                    diag.scroll_x = (tx.saturating_sub(vw / 2)) as u16;
                    diag.scroll_y = (ty.saturating_sub(vh / 2)) as u16;
                }
                diag.search_query.clear();
            }
        }

        // -- Vim state --
        Action::ClearPendingG => {
            state.vim.pending_g = false;
        }
        Action::SetPendingG => {
            state.vim.pending_g = true;
        }
        Action::ClearPendingD => {
            state.mutation.pending_d = false;
        }
        Action::SetPendingD => {
            state.mutation.pending_d = true;
        }
        Action::SetPendingLeader(v) => {
            state.vim.pending_leader = v;
        }

        // -- Status --
        Action::SetStatus(msg) => {
            state.status_msg = msg;
        }
        Action::SetError(msg) => {
            state.error_msg = msg;
        }
        Action::Quit => {
            state.should_quit = true;
        }
        Action::Noop => {}

        // -- Side effects --
        Action::SendCommand(cmd) => {
            let _ = cmd_tx.send(cmd);
        }
        Action::Batch(actions) => {
            for a in actions {
                apply(a, state, cmd_tx);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers used by apply()
// ---------------------------------------------------------------------------

fn apply_enter_cell_edit(state: &mut AppState, cmd_tx: &mpsc::UnboundedSender<CoreCommand>) {
    let row_idx = state.results.selected_row;
    let col_idx = state.results.selected_col;

    if state.results.data.columns.get(col_idx).is_none() {
        return;
    }
    if state.results.data.rows.get(row_idx).is_none() {
        return;
    }

    let sql = state.editor.sql();
    let parsed = crate::handlers::results::extract_schema_table_from_sql(&sql);
    tracing::info!("enter_cell_edit_mode: sql={:?} parsed={:?}", sql, parsed);
    let (schema, table_name) = parsed.unwrap_or_else(|| ("public".into(), "unknown".into()));

    state.mutation.pending_cell_edit = Some((row_idx, col_idx));
    tracing::info!("GetPrimaryKeys: schema={:?} table={:?}", schema, table_name);
    let _ = cmd_tx.send(CoreCommand::GetPrimaryKeys {
        schema,
        table: table_name,
    });
}

fn apply_stage_cell_edit(state: &mut AppState) {
    if let Some(ce) = state.mutation.cell_edit.take() {
        let new_val = ce.current_value();
        let col_name = ce.col_name.clone();
        if new_val != ce.original {
            state.mutation.pending_edits.insert(
                (ce.row_idx, ce.col_idx),
                PendingEdit {
                    new_val,
                    original: ce.original,
                    schema: ce.schema,
                    table: ce.table,
                    pk_col: ce.pk_col,
                    pk_val: ce.pk_val,
                    col_name: ce.col_name,
                },
            );
            let total =
                state.mutation.pending_edits.len() + state.mutation.pending_deletes.len();
            state.status_msg = Some(format!(
                "Staged edit on '{}'. Total staged: {}. Press Ctrl+W to commit.",
                col_name, total
            ));
        } else {
            state.status_msg = Some("No changes to stage (value unchanged).".into());
        }
    }
}

fn apply_commit_pending(state: &mut AppState, cmd_tx: &mpsc::UnboundedSender<CoreCommand>) {
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

fn apply_form_submit(state: &mut AppState, cmd_tx: &mpsc::UnboundedSender<CoreCommand>) {
    let form = &state.conn.form;

    if form.name.trim().is_empty() {
        state.conn.form.error = Some("Name is required".into());
        return;
    }

    let (config, password) = match form.backend {
        sbql_core::DbBackend::Postgres => {
            if form.host.trim().is_empty() {
                state.conn.form.error = Some("Host is required".into());
                return;
            }
            let port: u16 = match form.port.trim().parse() {
                Ok(p) => p,
                Err(_) => {
                    state.conn.form.error = Some("Port must be a number (1-65535)".into());
                    return;
                }
            };
            if form.user.trim().is_empty() {
                state.conn.form.error = Some("User is required".into());
                return;
            }
            if form.database.trim().is_empty() {
                state.conn.form.error = Some("Database is required".into());
                return;
            }

            let mut config = sbql_core::ConnectionConfig::new(
                form.name.trim(),
                form.host.trim(),
                port,
                form.user.trim(),
                form.database.trim(),
            );
            config.ssl_mode = form.ssl_mode.clone();

            if let Some(id) = form.editing_id {
                config.id = id;
            }

            let password = if form.password.is_empty() && form.editing_id.is_some() {
                None
            } else {
                Some(form.password.clone())
            };
            (config, password)
        }
        sbql_core::DbBackend::Sqlite => {
            if form.file_path.trim().is_empty() {
                state.conn.form.error = Some("File path is required".into());
                return;
            }

            let mut config =
                sbql_core::ConnectionConfig::new_sqlite(form.name.trim(), form.file_path.trim());

            if let Some(id) = form.editing_id {
                config.id = id;
            }

            (config, Some(String::new()))
        }
        sbql_core::DbBackend::Redis => {
            if form.host.trim().is_empty() {
                state.conn.form.error = Some("Host is required".into());
                return;
            }
            let port: u16 = match form.port.trim().parse() {
                Ok(p) => p,
                Err(_) => {
                    state.conn.form.error = Some("Port must be a number (1-65535)".into());
                    return;
                }
            };

            let mut config = sbql_core::ConnectionConfig::new_redis(
                form.name.trim(),
                form.host.trim(),
                port,
            );
            config.database = form.database.trim().to_string();

            if let Some(id) = form.editing_id {
                config.id = id;
            }

            let password = if form.password.is_empty() && form.editing_id.is_some() {
                None
            } else if form.password.is_empty() {
                Some(String::new())
            } else {
                Some(form.password.clone())
            };
            (config, password)
        }
    };

    let _ = cmd_tx.send(CoreCommand::SaveConnection { config, password });
    state.conn.form.visible = false;
    state.conn.form.error = None;
}

fn apply_refresh_filter_suggestions(
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
        state.filter.selected_suggestion = 0;
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
    state.filter.selected_suggestion = 0;
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

fn parse_filter_input(input: &str) -> Option<(String, &str)> {
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
        .get(state.filter.selected_suggestion)
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

/// Scroll the diagram viewport so the selected table stays visible (without centering).
fn diagram_keep_in_view(diag: &mut crate::app::DiagramState) {
    if let Some(&(tx, ty)) = diag.table_positions.get(&diag.selected_table) {
        let vw = diag.last_viewport_w as usize;
        let vh = diag.last_viewport_h as usize;
        let sx = diag.scroll_x as usize;
        let sy = diag.scroll_y as usize;
        // Horizontal keep-in-view
        if tx < sx {
            diag.scroll_x = tx as u16;
        } else if tx + 36 > sx + vw {
            diag.scroll_x = (tx + 36).saturating_sub(vw) as u16;
        }
        // Vertical keep-in-view
        if ty < sy {
            diag.scroll_y = ty as u16;
        } else if ty + 4 > sy + vh {
            diag.scroll_y = (ty + 4).saturating_sub(vh) as u16;
        }
    }
}

fn diagram_visible_table_indices(diag: &crate::app::DiagramState) -> Vec<usize> {
    let tables = &diag.data.tables;
    if tables.is_empty() {
        return Vec::new();
    }
    if !diag.focus_mode {
        return (0..tables.len()).collect();
    }

    let selected = diag.selected_table.min(tables.len().saturating_sub(1));
    let selected_key = tables[selected].qualified();
    let mut keys = std::collections::HashSet::new();
    keys.insert(selected_key.clone());

    for fk in &diag.data.foreign_keys {
        let from_key = format!("{}.{}", fk.from_schema, fk.from_table);
        let to_key = format!("{}.{}", fk.to_schema, fk.to_table);
        if from_key == selected_key {
            keys.insert(to_key);
        } else if to_key == selected_key {
            keys.insert(from_key);
        }
    }

    tables
        .iter()
        .enumerate()
        .filter_map(|(idx, t)| keys.contains(&t.qualified()).then_some(idx))
        .collect()
}

// Make parse_filter_input available for testing.
#[cfg(test)]
pub(crate) fn parse_filter_input_test(input: &str) -> Option<(String, &str)> {
    parse_filter_input(input)
}

/// Recompute autocomplete completions based on current editor state.
fn recompute_completions(state: &mut AppState) {
    let lines: Vec<String> = state.editor.textarea.lines().iter().map(|s| s.to_string()).collect();
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
            state.editor.completion.selected = 0;
            state.editor.completion.visible = true;
        }
    } else {
        state.editor.completion.dismiss();
    }
}

/// Apply live filter if the debounce deadline has passed. Called from tick.
/// Returns `true` if a filter was actually applied (state changed).
pub fn apply_live_filter_if_due(
    state: &mut AppState,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) -> bool {
    if !state.filter.visible {
        return false;
    }
    let Some(deadline) = state.filter.pending_live_apply_at else {
        return false;
    };
    if std::time::Instant::now() < deadline {
        return false;
    }
    state.filter.pending_live_apply_at = None;

    let query = state.filter.textarea.lines().join("");
    let trimmed = query.trim().to_owned();
    let is_candidate = parse_filter_input(&trimmed)
        .map(|(_, value)| !value.trim().is_empty())
        .unwrap_or(false);

    if !is_candidate {
        if state.filter.last_applied_query.is_some() {
            state.filter.last_applied_query = None;
            state.active_filter = None;
            let _ = cmd_tx.send(CoreCommand::ClearFilter);
            return true;
        }
        return false;
    }

    if state.filter.last_applied_query.as_deref() == Some(trimmed.as_str()) {
        return false;
    }

    state.filter.last_applied_query = Some(trimmed.clone());
    state.active_filter = Some(trimmed.clone());
    let _ = cmd_tx.send(CoreCommand::ApplyFilter { query: trimmed });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{
        AppState, DiagramGlyphMode, DiagramState, EditorMode, FocusedPanel, NavMode,
        PendingDelete, PendingEdit,
    };
    use crate::test_helpers::{cmd_channel, make_state_with_results};
    use sbql_core::{CoreCommand, DiagramData, QueryResult};

    // -----------------------------------------------------------------------
    // Navigation
    // -----------------------------------------------------------------------

    #[test]
    fn focus_panel_normal() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::FocusPanel(FocusedPanel::Results), &mut state, &tx);
        assert_eq!(state.focused, FocusedPanel::Results);
    }

    #[test]
    fn focus_panel_sidebar_hidden_redirects() {
        let mut state = AppState::new(vec![]);
        state.layout.sidebar_hidden = true;
        let (tx, _rx) = cmd_channel();
        apply(Action::FocusPanel(FocusedPanel::Connections), &mut state, &tx);
        assert_eq!(state.focused, FocusedPanel::Editor);
    }

    #[test]
    fn focus_panel_sidebar_hidden_tables_redirects() {
        let mut state = AppState::new(vec![]);
        state.layout.sidebar_hidden = true;
        let (tx, _rx) = cmd_channel();
        apply(Action::FocusPanel(FocusedPanel::Tables), &mut state, &tx);
        assert_eq!(state.focused, FocusedPanel::Editor);
    }

    #[test]
    fn set_nav_mode() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::SetNavMode(NavMode::Panel), &mut state, &tx);
        assert_eq!(state.vim.nav_mode, NavMode::Panel);
    }

    #[test]
    fn set_editor_mode() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::SetEditorMode(EditorMode::Insert), &mut state, &tx);
        assert_eq!(state.editor.mode, EditorMode::Insert);
    }

    #[test]
    fn toggle_sidebar_hides() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::ToggleSidebar, &mut state, &tx);
        assert!(state.layout.sidebar_hidden);
        assert_eq!(state.status_msg, Some("Sidebar hidden".into()));
    }

    #[test]
    fn toggle_sidebar_shows() {
        let mut state = AppState::new(vec![]);
        state.layout.sidebar_hidden = true;
        let (tx, _rx) = cmd_channel();
        apply(Action::ToggleSidebar, &mut state, &tx);
        assert!(!state.layout.sidebar_hidden);
        assert_eq!(state.status_msg, Some("Sidebar shown".into()));
    }

    #[test]
    fn toggle_sidebar_hidden_redirects_focus() {
        let mut state = AppState::new(vec![]);
        state.focused = FocusedPanel::Connections;
        let (tx, _rx) = cmd_channel();
        apply(Action::ToggleSidebar, &mut state, &tx);
        assert_eq!(state.focused, FocusedPanel::Editor);
    }

    // -----------------------------------------------------------------------
    // Results navigation
    // -----------------------------------------------------------------------

    #[test]
    fn move_row_down() {
        let mut state = make_state_with_results();
        let (tx, _rx) = cmd_channel();
        apply(Action::MoveRowDown, &mut state, &tx);
        assert_eq!(state.results.selected_row, 1);
    }

    #[test]
    fn move_row_down_triggers_page_fetch() {
        let mut state = make_state_with_results();
        state.results.data.has_next_page = true;
        state.results.selected_row = 4; // last row
        let (tx, mut rx) = cmd_channel();
        apply(Action::MoveRowDown, &mut state, &tx);
        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, CoreCommand::FetchPage { page: 1 }));
    }

    #[test]
    fn move_row_up() {
        let mut state = make_state_with_results();
        state.results.selected_row = 3;
        let (tx, _rx) = cmd_channel();
        apply(Action::MoveRowUp, &mut state, &tx);
        assert_eq!(state.results.selected_row, 2);
    }

    #[test]
    fn move_col_right() {
        let mut state = make_state_with_results();
        let (tx, _rx) = cmd_channel();
        apply(Action::MoveColRight, &mut state, &tx);
        assert_eq!(state.results.selected_col, 1);
    }

    #[test]
    fn move_col_left() {
        let mut state = make_state_with_results();
        state.results.selected_col = 2;
        let (tx, _rx) = cmd_channel();
        apply(Action::MoveColLeft, &mut state, &tx);
        assert_eq!(state.results.selected_col, 1);
    }

    #[test]
    fn move_row_first() {
        let mut state = make_state_with_results();
        state.results.selected_row = 3;
        let (tx, _rx) = cmd_channel();
        apply(Action::MoveRowFirst, &mut state, &tx);
        assert_eq!(state.results.selected_row, 0);
    }

    #[test]
    fn move_row_last() {
        let mut state = make_state_with_results();
        let (tx, _rx) = cmd_channel();
        apply(Action::MoveRowLast, &mut state, &tx);
        assert_eq!(state.results.selected_row, 4);
    }

    #[test]
    fn move_half_page_down() {
        let mut state = make_state_with_results();
        state.results.viewport_height = 4;
        let (tx, _rx) = cmd_channel();
        apply(Action::MoveHalfPageDown, &mut state, &tx);
        assert_eq!(state.results.selected_row, 2);
    }

    #[test]
    fn move_half_page_up() {
        let mut state = make_state_with_results();
        state.results.selected_row = 4;
        state.results.viewport_height = 4;
        let (tx, _rx) = cmd_channel();
        apply(Action::MoveHalfPageUp, &mut state, &tx);
        assert_eq!(state.results.selected_row, 2);
    }

    #[test]
    fn move_col_first() {
        let mut state = make_state_with_results();
        state.results.selected_col = 2;
        let (tx, _rx) = cmd_channel();
        apply(Action::MoveColFirst, &mut state, &tx);
        assert_eq!(state.results.selected_col, 0);
    }

    #[test]
    fn move_col_last() {
        let mut state = make_state_with_results();
        let (tx, _rx) = cmd_channel();
        apply(Action::MoveColLast, &mut state, &tx);
        assert_eq!(state.results.selected_col, 2);
    }

    #[test]
    fn set_results_row_in_bounds() {
        let mut state = make_state_with_results();
        let (tx, _rx) = cmd_channel();
        apply(Action::SetResultsRow(3), &mut state, &tx);
        assert_eq!(state.results.selected_row, 3);
    }

    #[test]
    fn set_results_row_out_of_bounds_no_change() {
        let mut state = make_state_with_results();
        let (tx, _rx) = cmd_channel();
        apply(Action::SetResultsRow(100), &mut state, &tx);
        assert_eq!(state.results.selected_row, 0);
    }

    #[test]
    fn set_results_col_clamped() {
        let mut state = make_state_with_results();
        let (tx, _rx) = cmd_channel();
        apply(Action::SetResultsCol(100), &mut state, &tx);
        assert_eq!(state.results.selected_col, 2); // max col = 2
    }

    // -----------------------------------------------------------------------
    // Cell edit
    // -----------------------------------------------------------------------

    #[test]
    fn cancel_cell_edit() {
        let mut state = make_state_with_results();
        state.mutation.cell_edit = Some(crate::app::CellEditState::new(
            0, 0, "id".into(), "1".into(), "public".into(), "users".into(), "id".into(), "1".into(),
        ));
        let (tx, _rx) = cmd_channel();
        apply(Action::CancelCellEdit, &mut state, &tx);
        assert!(state.mutation.cell_edit.is_none());
    }

    #[test]
    fn stage_cell_edit_with_change() {
        let mut state = make_state_with_results();
        let mut ce = crate::app::CellEditState::new(
            0, 0, "id".into(), "1".into(), "public".into(), "users".into(), "id".into(), "1".into(),
        );
        ce.textarea = tui_textarea::TextArea::default();
        ce.textarea.insert_str("999");
        state.mutation.cell_edit = Some(ce);
        let (tx, _rx) = cmd_channel();
        apply(Action::StageCellEdit, &mut state, &tx);
        assert!(state.mutation.cell_edit.is_none());
        assert_eq!(state.mutation.pending_edits.len(), 1);
    }

    #[test]
    fn stage_cell_edit_unchanged() {
        let mut state = make_state_with_results();
        let ce = crate::app::CellEditState::new(
            0, 0, "id".into(), "1".into(), "public".into(), "users".into(), "id".into(), "1".into(),
        );
        state.mutation.cell_edit = Some(ce);
        let (tx, _rx) = cmd_channel();
        apply(Action::StageCellEdit, &mut state, &tx);
        assert!(state.mutation.pending_edits.is_empty());
        assert_eq!(state.status_msg, Some("No changes to stage (value unchanged).".into()));
    }

    // -----------------------------------------------------------------------
    // Commit pending
    // -----------------------------------------------------------------------

    #[test]
    fn commit_pending_empty_shows_error() {
        let mut state = make_state_with_results();
        let (tx, _rx) = cmd_channel();
        apply(Action::CommitPending, &mut state, &tx);
        assert!(state.error_msg.is_some());
    }

    #[test]
    fn commit_pending_sends_commands() {
        let mut state = make_state_with_results();
        state.mutation.pending_edits.insert(
            (0, 1),
            PendingEdit {
                new_val: "new".into(),
                original: "old".into(),
                schema: "public".into(),
                table: "users".into(),
                pk_col: "id".into(),
                pk_val: "1".into(),
                col_name: "name".into(),
            },
        );
        state.mutation.pending_deletes.insert(
            2,
            PendingDelete {
                schema: "public".into(),
                table: "users".into(),
                pk_col: "id".into(),
                pk_val: "3".into(),
            },
        );
        let (tx, mut rx) = cmd_channel();
        apply(Action::CommitPending, &mut state, &tx);
        assert!(state.mutation.pending_edits.is_empty());
        assert!(state.mutation.pending_deletes.is_empty());
        // Should have sent UpdateCell, DeleteRow, FetchPage
        let mut cmds = Vec::new();
        while let Ok(cmd) = rx.try_recv() {
            cmds.push(cmd);
        }
        assert!(cmds.len() >= 3);
    }

    // -----------------------------------------------------------------------
    // Discard pending
    // -----------------------------------------------------------------------

    #[test]
    fn discard_pending_with_edits() {
        let mut state = make_state_with_results();
        state.mutation.pending_edits.insert(
            (0, 0),
            PendingEdit {
                new_val: "x".into(),
                original: "y".into(),
                schema: "p".into(),
                table: "t".into(),
                pk_col: "id".into(),
                pk_val: "1".into(),
                col_name: "c".into(),
            },
        );
        let (tx, _rx) = cmd_channel();
        apply(Action::DiscardPendingOrEsc, &mut state, &tx);
        assert!(state.mutation.pending_edits.is_empty());
        assert!(state.status_msg.unwrap().contains("discarded"));
    }

    #[test]
    fn discard_pending_empty_focuses_editor() {
        let mut state = make_state_with_results();
        state.focused = FocusedPanel::Results;
        let (tx, _rx) = cmd_channel();
        apply(Action::DiscardPendingOrEsc, &mut state, &tx);
        assert_eq!(state.focused, FocusedPanel::Editor);
    }

    // -----------------------------------------------------------------------
    // Editor
    // -----------------------------------------------------------------------

    #[test]
    fn run_query_sends_command() {
        let mut state = make_state_with_results();
        state.editor.textarea.insert_str("SELECT 1");
        let (tx, mut rx) = cmd_channel();
        apply(Action::RunQuery, &mut state, &tx);
        assert_eq!(state.focused, FocusedPanel::Results);
        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, CoreCommand::ExecuteQuery { .. }));
    }

    #[test]
    fn run_query_empty_noop() {
        let mut state = make_state_with_results();
        state.focused = FocusedPanel::Editor;
        let (tx, mut rx) = cmd_channel();
        apply(Action::RunQuery, &mut state, &tx);
        assert!(rx.try_recv().is_err());
        assert_eq!(state.focused, FocusedPanel::Editor);
    }

    // -----------------------------------------------------------------------
    // Connections
    // -----------------------------------------------------------------------

    #[test]
    fn select_connection() {
        let mut state = AppState::new(vec![
            sbql_core::ConnectionConfig::new("a", "h", 5432, "u", "d"),
            sbql_core::ConnectionConfig::new("b", "h", 5432, "u", "d"),
        ]);
        let (tx, _rx) = cmd_channel();
        apply(Action::SelectConnection(1), &mut state, &tx);
        assert_eq!(state.conn.selected, 1);
    }

    #[test]
    fn select_connection_clamped() {
        let mut state = AppState::new(vec![
            sbql_core::ConnectionConfig::new("a", "h", 5432, "u", "d"),
        ]);
        let (tx, _rx) = cmd_channel();
        apply(Action::SelectConnection(10), &mut state, &tx);
        assert_eq!(state.conn.selected, 0);
    }

    #[test]
    fn connect_selected_sends_command() {
        let cfg = sbql_core::ConnectionConfig::new("a", "h", 5432, "u", "d");
        let id = cfg.id;
        let mut state = AppState::new(vec![cfg]);
        let (tx, mut rx) = cmd_channel();
        apply(Action::ConnectSelected, &mut state, &tx);
        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, CoreCommand::Connect(cid) if cid == id));
    }

    #[test]
    fn open_new_form() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::OpenNewConnForm, &mut state, &tx);
        assert!(state.conn.form.visible);
    }

    #[test]
    fn open_edit_form() {
        let mut state = AppState::new(vec![
            sbql_core::ConnectionConfig::new("a", "localhost", 5432, "u", "d"),
        ]);
        let (tx, _rx) = cmd_channel();
        apply(Action::OpenEditConnForm, &mut state, &tx);
        assert!(state.conn.form.visible);
        assert_eq!(state.conn.form.name, "a");
    }

    #[test]
    fn init_delete_connection() {
        let mut state = AppState::new(vec![
            sbql_core::ConnectionConfig::new("myconn", "h", 5432, "u", "d"),
        ]);
        let (tx, _rx) = cmd_channel();
        apply(Action::InitDeleteConnection, &mut state, &tx);
        assert!(state.conn.pending_delete.is_some());
    }

    #[test]
    fn confirm_delete_connection() {
        let mut state = AppState::new(vec![]);
        state.conn.pending_delete = Some((uuid::Uuid::new_v4(), "test".into()));
        let (tx, mut rx) = cmd_channel();
        apply(Action::ConfirmDeleteConnection, &mut state, &tx);
        assert!(state.conn.pending_delete.is_none());
        assert!(rx.try_recv().is_ok());
    }

    #[test]
    fn cancel_delete_connection() {
        let mut state = AppState::new(vec![]);
        state.conn.pending_delete = Some((uuid::Uuid::new_v4(), "test".into()));
        let (tx, _rx) = cmd_channel();
        apply(Action::CancelDeleteConnection, &mut state, &tx);
        assert!(state.conn.pending_delete.is_none());
    }

    #[test]
    fn disconnect_active() {
        let mut state = AppState::new(vec![]);
        let id = uuid::Uuid::new_v4();
        state.conn.active_id = Some(id);
        let (tx, mut rx) = cmd_channel();
        apply(Action::DisconnectActive, &mut state, &tx);
        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, CoreCommand::Disconnect(d) if d == id));
    }

    // -----------------------------------------------------------------------
    // Connection form
    // -----------------------------------------------------------------------

    #[test]
    fn form_close() {
        let mut state = AppState::new(vec![]);
        state.conn.form.visible = true;
        let (tx, _rx) = cmd_channel();
        apply(Action::FormClose, &mut state, &tx);
        assert!(!state.conn.form.visible);
    }

    #[test]
    fn form_next_field_wraps() {
        let mut state = AppState::new(vec![]);
        state.conn.form.field_index = 7; // last PG field (SSL Mode)
        let (tx, _rx) = cmd_channel();
        apply(Action::FormNextField, &mut state, &tx);
        assert_eq!(state.conn.form.field_index, 0);
    }

    #[test]
    fn form_prev_field_wraps() {
        let mut state = AppState::new(vec![]);
        state.conn.form.field_index = 0;
        let (tx, _rx) = cmd_channel();
        apply(Action::FormPrevField, &mut state, &tx);
        assert_eq!(state.conn.form.field_index, 7); // wraps to last PG field
    }

    #[test]
    fn form_input_char() {
        let mut state = AppState::new(vec![]);
        state.conn.form.field_index = 1; // Name field
        let (tx, _rx) = cmd_channel();
        apply(Action::FormInput('a'), &mut state, &tx);
        assert_eq!(state.conn.form.name, "a");
    }

    #[test]
    fn form_backspace() {
        let mut state = AppState::new(vec![]);
        state.conn.form.name = "ab".into();
        state.conn.form.field_index = 1; // Name field
        let (tx, _rx) = cmd_channel();
        apply(Action::FormBackspace, &mut state, &tx);
        assert_eq!(state.conn.form.name, "a");
    }

    #[test]
    fn form_cycle_ssl() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::FormCycleSsl, &mut state, &tx);
        assert_eq!(state.conn.form.ssl_mode, sbql_core::SslMode::Require);
    }

    #[test]
    fn form_submit_valid() {
        let mut state = AppState::new(vec![]);
        state.conn.form.visible = true;
        state.conn.form.name = "test".into();
        state.conn.form.host = "localhost".into();
        state.conn.form.port = "5432".into();
        state.conn.form.user = "postgres".into();
        state.conn.form.database = "testdb".into();
        let (tx, mut rx) = cmd_channel();
        apply(Action::FormSubmit, &mut state, &tx);
        assert!(!state.conn.form.visible);
        assert!(rx.try_recv().is_ok());
    }

    #[test]
    fn form_submit_missing_name() {
        let mut state = AppState::new(vec![]);
        state.conn.form.visible = true;
        state.conn.form.name = "".into();
        let (tx, _rx) = cmd_channel();
        apply(Action::FormSubmit, &mut state, &tx);
        assert!(state.conn.form.error.is_some());
        assert!(state.conn.form.visible);
    }

    #[test]
    fn form_submit_bad_port() {
        let mut state = AppState::new(vec![]);
        state.conn.form.visible = true;
        state.conn.form.name = "test".into();
        state.conn.form.host = "localhost".into();
        state.conn.form.port = "not_a_number".into();
        state.conn.form.user = "u".into();
        state.conn.form.database = "d".into();
        let (tx, _rx) = cmd_channel();
        apply(Action::FormSubmit, &mut state, &tx);
        assert!(state.conn.form.error.unwrap().contains("Port"));
    }

    #[test]
    fn form_submit_redis_valid() {
        let mut state = AppState::new(vec![]);
        state.conn.form.visible = true;
        state.conn.form.backend = sbql_core::DbBackend::Redis;
        state.conn.form.name = "my-redis".into();
        state.conn.form.host = "localhost".into();
        state.conn.form.port = "6379".into();
        state.conn.form.database = "0".into();
        let (tx, mut rx) = cmd_channel();
        apply(Action::FormSubmit, &mut state, &tx);
        assert!(!state.conn.form.visible);
        assert!(state.conn.form.error.is_none());
        assert!(rx.try_recv().is_ok());
    }

    #[test]
    fn form_submit_redis_missing_host() {
        let mut state = AppState::new(vec![]);
        state.conn.form.visible = true;
        state.conn.form.backend = sbql_core::DbBackend::Redis;
        state.conn.form.name = "my-redis".into();
        state.conn.form.host = "".into();
        state.conn.form.port = "6379".into();
        let (tx, _rx) = cmd_channel();
        apply(Action::FormSubmit, &mut state, &tx);
        assert!(state.conn.form.visible);
        assert!(state.conn.form.error.as_ref().unwrap().contains("Host"));
    }

    #[test]
    fn form_submit_redis_bad_port() {
        let mut state = AppState::new(vec![]);
        state.conn.form.visible = true;
        state.conn.form.backend = sbql_core::DbBackend::Redis;
        state.conn.form.name = "my-redis".into();
        state.conn.form.host = "localhost".into();
        state.conn.form.port = "abc".into();
        let (tx, _rx) = cmd_channel();
        apply(Action::FormSubmit, &mut state, &tx);
        assert!(state.conn.form.visible);
        assert!(state.conn.form.error.as_ref().unwrap().contains("Port"));
    }

    // -----------------------------------------------------------------------
    // Tables
    // -----------------------------------------------------------------------

    #[test]
    fn select_table() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::SelectTable(5), &mut state, &tx);
        assert_eq!(state.tables.selected, 5);
    }

    // -----------------------------------------------------------------------
    // Filter
    // -----------------------------------------------------------------------

    #[test]
    fn open_filter() {
        let mut state = make_state_with_results();
        let (tx, _rx) = cmd_channel();
        apply(Action::OpenFilter, &mut state, &tx);
        assert!(state.filter.visible);
    }

    #[test]
    fn filter_close() {
        let mut state = make_state_with_results();
        state.filter.visible = true;
        let (tx, mut rx) = cmd_channel();
        apply(Action::FilterClose, &mut state, &tx);
        assert!(!state.filter.visible);
        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, CoreCommand::ClearFilter));
    }

    #[test]
    fn filter_close_suggestions() {
        let mut state = make_state_with_results();
        state.filter.show_suggestions = true;
        let (tx, _rx) = cmd_channel();
        apply(Action::FilterCloseSuggestions, &mut state, &tx);
        assert!(!state.filter.show_suggestions);
    }

    #[test]
    fn filter_suggestion_up() {
        let mut state = make_state_with_results();
        state.filter.selected_suggestion = 2;
        let (tx, _rx) = cmd_channel();
        apply(Action::FilterSuggestionUp, &mut state, &tx);
        assert_eq!(state.filter.selected_suggestion, 1);
    }

    #[test]
    fn filter_suggestion_down() {
        let mut state = make_state_with_results();
        state.filter.suggestions = vec!["a".into(), "b".into(), "c".into()];
        state.filter.selected_suggestion = 0;
        let (tx, _rx) = cmd_channel();
        apply(Action::FilterSuggestionDown, &mut state, &tx);
        assert_eq!(state.filter.selected_suggestion, 1);
    }

    #[test]
    fn filter_apply_with_query() {
        let mut state = make_state_with_results();
        state.filter.visible = true;
        state.filter.textarea = tui_textarea::TextArea::default();
        state.filter.textarea.insert_str("name:Alice");
        let (tx, mut rx) = cmd_channel();
        apply(Action::FilterApply, &mut state, &tx);
        assert!(!state.filter.visible);
        assert_eq!(state.active_filter, Some("name:Alice".into()));
        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, CoreCommand::ApplyFilter { .. }));
    }

    #[test]
    fn filter_apply_empty_clears() {
        let mut state = make_state_with_results();
        state.filter.visible = true;
        state.active_filter = Some("old".into());
        let (tx, mut rx) = cmd_channel();
        apply(Action::FilterApply, &mut state, &tx);
        assert!(state.active_filter.is_none());
        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, CoreCommand::ClearFilter));
    }

    // -----------------------------------------------------------------------
    // Diagram
    // -----------------------------------------------------------------------

    #[test]
    fn open_diagram_without_connection_shows_error() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::OpenDiagram, &mut state, &tx);
        assert!(state.error_msg.is_some());
    }

    #[test]
    fn open_diagram_with_connection_sends_command() {
        let mut state = AppState::new(vec![]);
        state.conn.active_id = Some(uuid::Uuid::new_v4());
        let (tx, mut rx) = cmd_channel();
        apply(Action::OpenDiagram, &mut state, &tx);
        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, CoreCommand::LoadDiagram));
    }

    #[test]
    fn close_diagram() {
        let mut state = AppState::new(vec![]);
        state.diagram = Some(DiagramState::new(DiagramData::default()));
        let (tx, _rx) = cmd_channel();
        apply(Action::CloseDiagram, &mut state, &tx);
        assert!(state.diagram.is_none());
    }

    #[test]
    fn diagram_scroll() {
        let mut state = AppState::new(vec![]);
        state.diagram = Some(DiagramState::new(DiagramData::default()));
        let (tx, _rx) = cmd_channel();
        apply(Action::DiagramScroll { dx: 5, dy: 3 }, &mut state, &tx);
        assert_eq!(state.diagram.as_ref().unwrap().scroll_x, 5);
        assert_eq!(state.diagram.as_ref().unwrap().scroll_y, 3);
    }

    #[test]
    fn diagram_toggle_glyph() {
        let mut state = AppState::new(vec![]);
        state.diagram = Some(DiagramState::new(DiagramData::default()));
        let (tx, _rx) = cmd_channel();
        apply(Action::DiagramToggleGlyph, &mut state, &tx);
        assert_eq!(
            state.diagram.as_ref().unwrap().glyph_mode,
            DiagramGlyphMode::Unicode
        );
    }

    #[test]
    fn diagram_toggle_focus() {
        let mut state = AppState::new(vec![]);
        state.diagram = Some(DiagramState::new(DiagramData::default()));
        let (tx, _rx) = cmd_channel();
        apply(Action::DiagramToggleFocus, &mut state, &tx);
        assert!(state.diagram.as_ref().unwrap().focus_mode);
    }

    // -----------------------------------------------------------------------
    // Vim state
    // -----------------------------------------------------------------------

    #[test]
    fn set_pending_g() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::SetPendingG, &mut state, &tx);
        assert!(state.vim.pending_g);
    }

    #[test]
    fn clear_pending_g() {
        let mut state = AppState::new(vec![]);
        state.vim.pending_g = true;
        let (tx, _rx) = cmd_channel();
        apply(Action::ClearPendingG, &mut state, &tx);
        assert!(!state.vim.pending_g);
    }

    #[test]
    fn set_pending_d() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::SetPendingD, &mut state, &tx);
        assert!(state.mutation.pending_d);
    }

    #[test]
    fn clear_pending_d() {
        let mut state = AppState::new(vec![]);
        state.mutation.pending_d = true;
        let (tx, _rx) = cmd_channel();
        apply(Action::ClearPendingD, &mut state, &tx);
        assert!(!state.mutation.pending_d);
    }

    #[test]
    fn set_pending_leader() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::SetPendingLeader(true), &mut state, &tx);
        assert!(state.vim.pending_leader);
    }

    // -----------------------------------------------------------------------
    // Status
    // -----------------------------------------------------------------------

    #[test]
    fn set_status() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::SetStatus(Some("hello".into())), &mut state, &tx);
        assert_eq!(state.status_msg, Some("hello".into()));
    }

    #[test]
    fn set_error() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::SetError(Some("err".into())), &mut state, &tx);
        assert_eq!(state.error_msg, Some("err".into()));
    }

    #[test]
    fn quit() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::Quit, &mut state, &tx);
        assert!(state.should_quit);
    }

    #[test]
    fn noop_does_nothing() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(Action::Noop, &mut state, &tx);
        assert!(!state.should_quit);
    }

    #[test]
    fn batch_applies_all() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        apply(
            Action::Batch(vec![
                Action::SetPendingG,
                Action::SetPendingD,
            ]),
            &mut state,
            &tx,
        );
        assert!(state.vim.pending_g);
        assert!(state.mutation.pending_d);
    }

    #[test]
    fn send_command() {
        let mut state = AppState::new(vec![]);
        let (tx, mut rx) = cmd_channel();
        apply(
            Action::SendCommand(CoreCommand::ListTables),
            &mut state,
            &tx,
        );
        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, CoreCommand::ListTables));
    }

    // -----------------------------------------------------------------------
    // parse_filter_input
    // -----------------------------------------------------------------------

    #[test]
    fn parse_filter_col_value() {
        let result = parse_filter_input_test("name:Alice");
        assert_eq!(result, Some(("name".into(), "Alice")));
    }

    #[test]
    fn parse_filter_empty_col() {
        let result = parse_filter_input_test(":value");
        assert!(result.is_none());
    }

    #[test]
    fn parse_filter_no_colon() {
        let result = parse_filter_input_test("plain text");
        assert!(result.is_none());
    }

    // -----------------------------------------------------------------------
    // apply_live_filter_if_due
    // -----------------------------------------------------------------------

    #[test]
    fn live_filter_not_visible_returns_false() {
        let mut state = AppState::new(vec![]);
        let (tx, _rx) = cmd_channel();
        assert!(!apply_live_filter_if_due(&mut state, &tx));
    }

    #[test]
    fn live_filter_no_deadline_returns_false() {
        let mut state = AppState::new(vec![]);
        state.filter.visible = true;
        let (tx, _rx) = cmd_channel();
        assert!(!apply_live_filter_if_due(&mut state, &tx));
    }

    #[test]
    fn live_filter_deadline_in_future_returns_false() {
        let mut state = AppState::new(vec![]);
        state.filter.visible = true;
        state.filter.pending_live_apply_at =
            Some(std::time::Instant::now() + std::time::Duration::from_secs(60));
        let (tx, _rx) = cmd_channel();
        assert!(!apply_live_filter_if_due(&mut state, &tx));
    }
}
