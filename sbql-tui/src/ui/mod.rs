pub mod cell_edit;
pub mod connections;
pub mod diagram;
pub mod editor;
pub mod layout;
pub mod results;
pub mod theme;

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{AppState, EditorMode, LastAreas, NavMode};

/// Root draw function — dispatches to each panel.
pub fn draw(frame: &mut Frame, state: &mut AppState) {
    // Diagram mode replaces the entire layout when active.
    if let Some(ref mut diag) = state.diagram {
        diagram::draw(frame, diag);
        return;
    }

    let areas = layout::compute(frame.area(), state.layout.sidebar_hidden);

    // Save layout so mouse handler can do accurate hit-testing
    state.layout.last_areas = Some(LastAreas {
        conn_list: areas.connections,
        table_list: areas.tables,
        editor: areas.editor,
        results: areas.results,
    });

    if !state.layout.sidebar_hidden {
        connections::draw_connections(frame, &state.conn, state.focused, areas.connections);
        connections::draw_tables(
            frame,
            &state.tables,
            state.focused,
            state.results.is_loading,
            areas.tables,
        );
    }
    editor::draw(
        frame,
        &mut state.editor,
        &state.conn,
        state.focused,
        areas.editor,
    );
    let output = results::draw(
        frame,
        &state.results,
        &state.mutation,
        state.focused,
        state.active_filter.as_deref(),
        state.filter.visible,
        state.layout.spinner_frame,
        state.conn.active_id.is_some(),
        areas.results,
    );
    // Write back computed values from the draw cycle
    state.results.viewport_height = output.viewport_height;
    state.results.viewport_cols = output.viewport_cols;
    state.layout.last_col_widths = output.col_widths.clone();
    state.results.cached_col_widths = output.col_widths;
    state.results.col_widths_dirty = false;

    // Overlays (drawn on top)
    if state.conn.form.visible {
        connections::draw_form(frame, &state.conn.form, frame.area());
    }

    if let Some(ref mut ce) = state.mutation.cell_edit {
        cell_edit::draw(frame, ce, &state.layout, &state.results, frame.area());
    }

    if state.filter.visible {
        results::draw_filter_bar(frame, &mut state.filter, areas.results);
    }

    // Status bar — always visible at the bottom
    draw_status_bar(frame, state, areas.status_bar);
}

fn draw_status_bar(frame: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    if let Some((_, ref name)) = state.conn.pending_delete {
        let bar = Paragraph::new(Line::from(Span::styled(
            format!(" ! Delete connection '{name}'? y/Enter confirm, n/Esc cancel"),
            Style::default()
                .fg(theme::BASE)
                .bg(theme::YELLOW)
                .add_modifier(Modifier::BOLD),
        )));
        frame.render_widget(bar, area);
    } else if let Some(ref err) = state.error_msg {
        let bar = Paragraph::new(Line::from(Span::styled(
            format!(" ✗ {err}"),
            Style::default()
                .fg(theme::TEXT)
                .bg(theme::RED)
                .add_modifier(Modifier::BOLD),
        )));
        frame.render_widget(bar, area);
    } else if let Some(ref msg) = state.status_msg {
        let bar = Paragraph::new(Line::from(Span::styled(
            format!(" ✓ {msg}"),
            Style::default().fg(theme::BASE).bg(theme::GREEN),
        )));
        frame.render_widget(bar, area);
    } else {
        const SPINNER: [&str; 8] = ["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];
        let mode = match (state.vim.nav_mode, state.editor.mode) {
            (_, EditorMode::Insert) => "INSERT",
            (NavMode::Global, EditorMode::Normal) => "GLOBAL",
            (NavMode::Panel, EditorMode::Normal) => "PANEL",
        };
        let leader = if state.vim.pending_leader {
            "  Leader: _"
        } else {
            ""
        };
        let help = format!(
            " sbql [{mode}]  q/Ctrl+C: quit  hjkl: panels(global)  Enter: panel mode  Esc: global  Tab: cycle  SPC e: sidebar  i: insert/edit  ^S/F5: run{leader}"
        );
        let line = if state.results.is_loading {
            let frame_char = SPINNER[state.layout.spinner_frame % SPINNER.len()];
            Line::from(vec![
                Span::styled(help, Style::default().fg(theme::OVERLAY0)),
                Span::styled(
                    format!("  {frame_char} "),
                    Style::default().fg(theme::YELLOW),
                ),
            ])
        } else {
            Line::from(Span::styled(help, Style::default().fg(theme::OVERLAY0)))
        };
        frame.render_widget(Paragraph::new(line), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use ratatui::{backend::TestBackend, Terminal};
    use sbql_core::{CoreEvent, QueryResult};

    #[test]
    fn test_ui_draws_query_results() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(vec![]);

        let result = QueryResult {
            columns: vec!["id".into(), "username".into()],
            rows: vec![
                vec!["1".into(), "alice_test_user".into()],
                vec!["2".into(), "bob_test_user".into()],
            ],
            page: 0,
            has_next_page: false,
        };
        state.apply_core_event(CoreEvent::QueryResult(result));

        terminal.draw(|f| draw(f, &mut state)).unwrap();

        let buffer = terminal.backend().buffer();
        let mut content = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                let cell = buffer.cell((x, y)).unwrap();
                content.push_str(cell.symbol());
            }
            content.push('\n');
        }

        assert!(
            content.contains("username"),
            "Column 'username' should be rendered"
        );
        assert!(
            content.contains("alice_test_user"),
            "Row data 'alice_test_user' should be rendered"
        );
        assert!(
            content.contains("bob_test_user"),
            "Row data 'bob_test_user' should be rendered"
        );
    }

    #[test]
    fn test_ui_status_bar_rendering() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(vec![]);

        state.apply_core_event(CoreEvent::Connected(uuid::Uuid::new_v4()));

        terminal.draw(|f| draw(f, &mut state)).unwrap();
        let buffer = terminal.backend().buffer();

        let mut content = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                content.push_str(buffer.cell((x, y)).unwrap().symbol());
            }
        }

        assert!(
            content.contains("Connected to "),
            "Status bar should show connection success"
        );
    }
}
