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

use crate::app::{AppState, LastAreas};

/// Root draw function — dispatches to each panel.
pub fn draw(frame: &mut Frame, state: &mut AppState) {
    // Diagram mode replaces the entire layout when active.
    if let Some(ref mut diag) = state.diagram {
        diagram::draw(frame, diag);
        return;
    }

    let areas = layout::compute(frame.area(), state.sidebar_hidden);

    // Save layout so mouse handler can do accurate hit-testing
    state.last_areas = Some(LastAreas {
        conn_list: areas.connections,
        table_list: areas.tables,
        editor: areas.editor,
        results: areas.results,
    });

    if !state.sidebar_hidden {
        connections::draw_connections(frame, state, areas.connections);
        connections::draw_tables(frame, state, areas.tables);
    }
    editor::draw(frame, state, areas.editor);
    results::draw(frame, state, areas.results);

    // Overlays (drawn on top)
    if state.conn_form.visible {
        connections::draw_form(frame, state);
    }

    if let Some(ref _ce) = state.cell_edit {
        cell_edit::draw(frame, state);
    }

    if state.filter_bar.visible {
        results::draw_filter_bar(frame, state, areas.results);
    }

    // Status bar — always visible at the bottom
    draw_status_bar(frame, state, areas.status_bar);
}

fn draw_status_bar(frame: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    if let Some(ref err) = state.error_msg {
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
        let help = " sbql  q/Ctrl+C: quit  M-hjkl: panels  Tab: cycle  ^B: sidebar  i: insert  Esc: normal  ^S/F5: run";
        let line = if state.is_loading {
            let frame_char = SPINNER[state.spinner_frame % SPINNER.len()];
            Line::from(vec![
                Span::styled(help, Style::default().fg(theme::OVERLAY0)),
                Span::styled(
                    format!("  {frame_char} "),
                    Style::default().fg(theme::YELLOW),
                ),
            ])
        } else {
            Line::from(Span::styled(
                format!("{help} "),
                Style::default().fg(theme::OVERLAY0),
            ))
        };
        frame.render_widget(Paragraph::new(line), area);
    }
}
