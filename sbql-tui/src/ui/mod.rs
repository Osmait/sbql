pub mod cache;
pub mod cell_edit;
pub mod connections;
pub mod diagram;
pub mod editor;
pub mod hit;
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

use crate::app::{AppState, EditorMode, FocusedPanel, NavMode};
use crate::notice::Level;
use crate::ui::hit::Zone;

/// Root draw function — dispatches to each panel.
///
/// Also rebuilds the hit map: every panel registers what it painted, in paint
/// order, so the mouse handler resolves a click the same way the screen does.
pub fn draw(frame: &mut Frame, state: &mut AppState, cache: &mut cache::RenderCache) {
    // A zone must never outlive the frame that drew it.
    state.layout.hits.clear();

    // Diagram mode replaces the entire layout when active.
    if let Some(ref mut diag) = state.diagram {
        // Measure first: this settles the canvas cache and clamps scroll, so
        // rendering below is a pure read.
        let full = frame.area();
        diagram::measure(diag, cache, full);
        diagram::draw(frame, diag, cache, &mut state.layout.hits);
        return;
    }

    let areas = layout::compute(frame.area(), state.layout.sidebar_hidden);

    state.layout.results_area = Some(areas.results);

    // Panel bodies first: they are the fallback under everything each panel
    // registers on top of them.
    if !state.layout.sidebar_hidden {
        state
            .layout
            .hits
            .register(areas.connections, Zone::Panel(FocusedPanel::Connections));
        state
            .layout
            .hits
            .register(areas.tables, Zone::Panel(FocusedPanel::Tables));
    }
    state
        .layout
        .hits
        .register(areas.editor, Zone::Panel(FocusedPanel::Editor));
    state
        .layout
        .hits
        .register(areas.results, Zone::Panel(FocusedPanel::Results));

    if !state.layout.sidebar_hidden {
        connections::draw_connections(
            frame,
            &state.conn,
            state.focused,
            areas.connections,
            &mut state.layout.hits,
        );
        connections::draw_tables(
            frame,
            &state.tables,
            state.focused,
            state.results.is_loading,
            areas.tables,
            &mut state.layout.hits,
        );
    }
    // The diagram is closed, so a canvas from a previous one must not linger.
    cache.clear_diagram_canvas();

    // The text sits inside the border. Registered here rather than inside the
    // editor's draw so the rect is also kept for drags that leave the panel.
    let editor_text = areas.editor.inner(ratatui::layout::Margin::new(1, 1));
    state.layout.editor_text_rect = Some(editor_text);
    state.layout.hits.register(editor_text, Zone::EditorText);

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
    results::draw(
        frame,
        &results_view,
        &results_layout,
        areas.results,
        &mut state.layout.hits,
    );

    // Overlays (drawn on top)
    if state.conn.form.visible {
        connections::draw_form(
            frame,
            &state.conn.form,
            frame.area(),
            &mut state.layout.hits,
        );
    }

    if let Some(ref mut ce) = state.mutation.cell_edit {
        let mut hits = std::mem::take(&mut state.layout.hits);
        cell_edit::draw(
            frame,
            ce,
            &state.layout,
            &state.results,
            frame.area(),
            &mut hits,
        );
        state.layout.hits = hits;
    }

    if state.filter.visible {
        let mut hits = std::mem::take(&mut state.layout.hits);
        results::draw_filter_bar(frame, &mut state.filter, areas.results, &mut hits);
        state.layout.hits = hits;
    }

    // Drawn last so it sits over everything else, including the other overlays.
    if state.notice_detail_open {
        if let Some(ref n) = state.notice {
            notice::draw(frame, n, frame.area(), &mut state.layout.hits);
        }
    }

    // Status bar — always visible at the bottom. Drawn from a detached hit map
    // because it reads the whole state while registering its own zones.
    let mut hits = std::mem::take(&mut state.layout.hits);
    draw_status_bar(frame, state, areas.status_bar, &mut hits);
    state.layout.hits = hits;
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

fn draw_status_bar(
    frame: &mut Frame,
    state: &AppState,
    area: ratatui::layout::Rect,
    hits: &mut hit::HitMap,
) {
    if let Some((_, ref name)) = state.conn.pending_delete {
        let style = Style::default()
            .fg(theme::BASE)
            .bg(theme::YELLOW)
            .add_modifier(Modifier::BOLD);
        // Laid out as parts so the two answers can be clicked. Built by walking
        // the widths rather than counting characters by hand, which is how a
        // hit region silently drifts off its own label.
        let parts = [
            (format!(" ! Delete connection '{name}'? "), None),
            ("y/Enter confirm".to_owned(), Some(Zone::ConfirmDeleteYes)),
            (", ".to_owned(), None),
            ("n/Esc cancel".to_owned(), Some(Zone::ConfirmDeleteNo)),
        ];
        let mut x = area.x;
        let mut spans = Vec::with_capacity(parts.len());
        for (text, zone) in parts {
            let width = u16::try_from(text.chars().count()).unwrap_or(u16::MAX);
            if let Some(zone) = zone {
                hits.register(
                    ratatui::layout::Rect::new(x, area.y, width.min(area.right() - x), 1),
                    zone,
                );
            }
            x = x.saturating_add(width).min(area.right());
            spans.push(Span::styled(text, style));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
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

        // Clicking the affordance does what it says, the same as Ctrl+E.
        if !more.is_empty() {
            let used = u16::try_from(text.chars().count()).unwrap_or(u16::MAX);
            let width = u16::try_from(more.chars().count()).unwrap_or(u16::MAX);
            let x = area.x.saturating_add(used);
            if x < area.right() {
                hits.register(
                    ratatui::layout::Rect::new(x, area.y, width.min(area.right() - x), 1),
                    Zone::NoticeDetailHint,
                );
            }
        }

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
        // Panel navigation leads, because it is what a new user reaches for
        // first and what three separate bindings already did invisibly.
        let help = format!(
            " sbql [{mode}]  ^hjkl: panels  ^1-4: jump  Tab: cycle  i: insert/edit  ^S/F5: run  Esc: global  q/^C: quit{leader}"
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
