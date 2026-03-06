//! Left panels — connection list, table list, and add/edit form overlay.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::{AppState, ConnectionForm, FocusedPanel};
use crate::ui::theme;

// ---------------------------------------------------------------------------
// Connections panel (top-left)
// ---------------------------------------------------------------------------

pub fn draw_connections(frame: &mut Frame, state: &AppState, area: Rect) {
    let is_focused = state.focused == FocusedPanel::Connections;

    let conn_items: Vec<ListItem> = state
        .connections
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let is_active = state.active_connection_id == Some(c.id);
            let indicator = if is_active { "● " } else { "  " };
            let style = if i == state.selected_connection && is_focused {
                Style::default()
                    .fg(theme::BASE)
                    .bg(theme::BLUE)
                    .add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default().fg(theme::GREEN)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    indicator,
                    Style::default().fg(if is_active {
                        theme::GREEN
                    } else {
                        theme::OVERLAY0
                    }),
                ),
                Span::styled(c.name.clone(), style),
            ]))
        })
        .collect();

    let conn_title = if is_focused {
        " Connections (Enter=connect  n=new) "
    } else if state.connections.is_empty() {
        " Connections (n=new) "
    } else {
        " Connections "
    };

    let border_style = if is_focused {
        Style::default().fg(theme::BLUE)
    } else {
        Style::default().fg(theme::OVERLAY0)
    };

    let conn_list = List::new(conn_items)
        .block(
            Block::default()
                .title(conn_title)
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    let mut conn_state = ListState::default();
    conn_state.select(Some(state.selected_connection));
    frame.render_stateful_widget(conn_list, area, &mut conn_state);
}

// ---------------------------------------------------------------------------
// Tables panel (bottom-left)
// ---------------------------------------------------------------------------

pub fn draw_tables(frame: &mut Frame, state: &AppState, area: Rect) {
    let is_focused = state.focused == FocusedPanel::Tables;

    let border_style = if is_focused {
        Style::default().fg(theme::BLUE)
    } else {
        Style::default().fg(theme::OVERLAY0)
    };

    let table_items: Vec<ListItem> = state
        .tables
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let style = if i == state.selected_table && is_focused {
                Style::default()
                    .fg(theme::BASE)
                    .bg(theme::YELLOW)
                    .add_modifier(Modifier::BOLD)
            } else if i == state.selected_table {
                Style::default().fg(theme::YELLOW)
            } else {
                Style::default().fg(theme::OVERLAY2)
            };
            ListItem::new(Span::styled(t.qualified(), style))
        })
        .collect();

    let table_title = if state.is_loading && state.tables.is_empty() {
        " Tables (loading...) "
    } else if is_focused {
        " Tables (Enter=SELECT *  Esc=exit) "
    } else {
        " Tables "
    };

    let table_list = List::new(table_items).block(
        Block::default()
            .title(table_title)
            .borders(Borders::ALL)
            .border_style(border_style),
    );

    let mut tbl_state = ListState::default();
    tbl_state.select(if state.tables.is_empty() {
        None
    } else {
        Some(state.selected_table)
    });
    frame.render_stateful_widget(table_list, area, &mut tbl_state);
}

// ---------------------------------------------------------------------------
// Connection form overlay
// ---------------------------------------------------------------------------

pub fn draw_form(frame: &mut Frame, state: &AppState) {
    let area = centered_rect(60, 70, frame.area());

    // Clear the background
    frame.render_widget(Clear, area);

    let title = if state.conn_form.editing_id.is_some() {
        " Edit Connection "
    } else {
        " New Connection "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BLUE));

    frame.render_widget(block.clone(), area);
    let inner = block.inner(area);

    let field_count = ConnectionForm::field_count();
    let mut constraints = vec![Constraint::Length(3); field_count];
    constraints.push(Constraint::Min(1)); // spacer
    constraints.push(Constraint::Length(1)); // help line

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for i in 0..field_count {
        let label = ConnectionForm::field_label(i);
        let is_active = state.conn_form.field_index == i;
        let border_style = if is_active {
            Style::default().fg(theme::BLUE)
        } else {
            Style::default().fg(theme::OVERLAY0)
        };
        let title_style = if is_active {
            Style::default()
                .fg(theme::BLUE)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::OVERLAY2)
        };

        if i == 6 {
            // SSL Mode is a cycle selector, not a text field
            let ssl_display = state.conn_form.ssl_mode.as_str().to_owned();
            let hint = if is_active { "  Space: cycle" } else { "" };
            let para = Paragraph::new(format!("{ssl_display}{hint}")).block(
                Block::default()
                    .title(Span::styled(format!(" {label} "), title_style))
                    .borders(Borders::ALL)
                    .border_style(border_style),
            );
            frame.render_widget(para, chunks[i]);
            continue;
        }

        let value = match i {
            0 => &state.conn_form.name,
            1 => &state.conn_form.host,
            2 => &state.conn_form.port,
            3 => &state.conn_form.user,
            4 => &state.conn_form.database,
            5 => &state.conn_form.password,
            _ => continue,
        };
        // Mask the password field; show a placeholder when editing with blank password
        let display = if i == 5 {
            if value.is_empty() && state.conn_form.editing_id.is_some() {
                "(unchanged)".to_owned()
            } else {
                "*".repeat(value.len())
            }
        } else {
            value.clone()
        };

        let para = Paragraph::new(display).block(
            Block::default()
                .title(Span::styled(format!(" {label} "), title_style))
                .borders(Borders::ALL)
                .border_style(border_style),
        );
        frame.render_widget(para, chunks[i]);
    }

    // Help line
    let help = Paragraph::new("Tab/↑↓: next field  Space: cycle SSL  Enter: save  Esc: cancel")
        .style(Style::default().fg(theme::OVERLAY0));
    frame.render_widget(help, *chunks.last().unwrap());

    // Error message
    if let Some(ref err) = state.conn_form.error {
        let err_area = Rect {
            y: inner.y + inner.height.saturating_sub(2),
            height: 1,
            ..inner
        };
        frame.render_widget(
            Paragraph::new(err.as_str()).style(Style::default().fg(theme::RED)),
            err_area,
        );
    }
}

// ---------------------------------------------------------------------------
// Helper: centered rect
// ---------------------------------------------------------------------------

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
