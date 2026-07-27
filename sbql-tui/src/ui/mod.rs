pub mod cache;
pub mod cell_edit;
pub mod connections;
pub mod diagram;
pub mod editor;
pub mod layout;
pub mod notice;
pub mod results;
pub mod theme;

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{AppState, EditorMode, LastAreas, NavMode};
use crate::notice::Level;

/// Root draw function — dispatches to each panel.
pub fn draw(frame: &mut Frame, state: &mut AppState, cache: &mut cache::RenderCache) {
    // Diagram mode replaces the entire layout when active.
    if let Some(ref mut diag) = state.diagram {
        // Measure first: this settles the canvas cache and clamps scroll, so
        // rendering below is a pure read.
        let full = frame.area();
        diagram::measure(diag, cache, full);
        diagram::draw(frame, diag, cache);
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
    // The diagram is closed, so a canvas from a previous one must not linger.
    cache.clear_diagram_canvas();

    editor::draw(
        frame,
        cache,
        &mut state.editor,
        &state.conn,
        state.focused,
        areas.editor,
    );
    // Measure first, then render. Drawing no longer produces state the caller
    // has to copy back out of it.
    let results_layout = results::measure(&state.results, areas.results);
    state.results.viewport_height = results_layout.viewport_height;
    state.results.viewport_cols = results_layout.viewport_cols;
    state.layout.last_col_widths = results_layout.col_widths.clone();
    state.results.cached_col_widths = results_layout.col_widths.clone();
    state.results.col_widths_dirty = false;

    let results_view = results::ResultsView {
        results: &state.results,
        mutation: &state.mutation,
        focused: state.focused,
        active_filter: state.active_filter.as_deref(),
        filter_visible: state.filter.visible,
        spinner_frame: state.layout.spinner_frame,
        has_active_connection: state.conn.active_id.is_some(),
    };
    results::draw(frame, &results_view, &results_layout, areas.results);

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

    // Drawn last so it sits over everything else, including the other overlays.
    if state.notice_detail_open {
        if let Some(ref n) = state.notice {
            notice::draw(frame, n, frame.area());
        }
    }

    // Status bar — always visible at the bottom
    draw_status_bar(frame, state, areas.status_bar);
}

/// Fit `text` to `width`, and say so when it does not fit.
///
/// The bar is one row high with no wrapping, so anything too long used to be
/// cut off with no sign that there was more — which, for a database client,
/// is exactly what happens to the interesting half of a SQL error. Now the
/// truncation is visible and Ctrl+E has the rest.
fn fit(text: &str, width: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= width {
        return text.to_owned();
    }
    // Room for the ellipsis; a width this small has nothing to show anyway.
    let keep = width.saturating_sub(1);
    chars.into_iter().take(keep).chain(['…']).collect()
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
    } else if let Some(ref notice) = state.notice {
        let (marker, style) = match notice.level {
            Level::Error => (
                "✗",
                Style::default()
                    .fg(theme::TEXT)
                    .bg(theme::RED)
                    .add_modifier(Modifier::BOLD),
            ),
            Level::Warning => (
                "!",
                Style::default()
                    .fg(theme::BASE)
                    .bg(theme::YELLOW)
                    .add_modifier(Modifier::BOLD),
            ),
            Level::Info => ("✓", Style::default().fg(theme::BASE).bg(theme::GREEN)),
        };

        let more = if notice.has_detail() {
            "  (Ctrl+E: details)"
        } else {
            ""
        };
        // The affordance is the part that must survive: a truncated message the
        // user cannot expand is worse than one they know how to expand.
        let room = (area.width as usize).saturating_sub(more.chars().count());
        let text = fit(&format!(" {marker} {}", notice.text), room);

        let bar = Paragraph::new(Line::from(vec![
            Span::styled(text, style),
            Span::styled(more, style),
        ]));
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
            total_count: None,
        };
        state.apply_core_event(CoreEvent::QueryResult(result));

        let mut cache = cache::RenderCache::new();
        terminal.draw(|f| draw(f, &mut state, &mut cache)).unwrap();

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

        let mut cache = cache::RenderCache::new();
        terminal.draw(|f| draw(f, &mut state, &mut cache)).unwrap();
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

    // -- fit --

    #[test]
    fn a_message_that_fits_is_left_alone() {
        assert_eq!(fit("short", 20), "short");
        assert_eq!(fit("exactly-ten", 11), "exactly-ten");
    }

    /// The bar cannot wrap, so the cut has to be visible. Silently dropping the
    /// end of a database error is how the useful half goes missing.
    #[test]
    fn a_message_too_long_is_marked_as_cut() {
        let cut = fit("syntax error at or near \")\"", 10);

        assert_eq!(cut.chars().count(), 10, "must fit the width exactly");
        assert!(cut.ends_with('…'), "{cut}");
        assert!(cut.starts_with("syntax"), "{cut}");
    }

    #[test]
    fn fitting_into_almost_no_room_does_not_panic() {
        for width in 0..3 {
            let cut = fit("something", width);
            assert!(cut.chars().count() <= width.max(1));
        }
    }

    /// Counted in characters, not bytes: a multi-byte name must not be sliced
    /// down the middle of a code point.
    #[test]
    fn fitting_counts_characters_not_bytes() {
        let cut = fit("préférences très longues", 8);
        assert_eq!(cut.chars().count(), 8, "{cut}");
    }

    /// A failure has to reach the bar, in its own colour, with the way to read
    /// the rest of it.
    #[test]
    fn a_failure_reaches_the_status_bar_with_its_detail_offered() {
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
        let mut state = AppState::new(vec![]);
        let mut cache = cache::RenderCache::new();

        state.apply_core_event(CoreEvent::Error(sbql_core::CoreError::new(
            sbql_core::ErrorKind::NoActiveConnection,
            "No active connection",
        )));

        terminal.draw(|f| draw(f, &mut state, &mut cache)).unwrap();
        let buffer = terminal.backend().buffer();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains("No active connection"), "{content}");
        assert!(
            content.contains("Ctrl+E"),
            "the way to read the hint should be offered"
        );
    }
}
