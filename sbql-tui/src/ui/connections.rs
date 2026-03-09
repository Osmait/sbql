//! Left panels — connection list, table list, and add/edit form overlay.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::{ConnectionForm, ConnectionState, FocusedPanel, TableBrowserState};
use crate::ui::theme;

// ---------------------------------------------------------------------------
// Connections panel (top-left)
// ---------------------------------------------------------------------------

pub fn draw_connections(
    frame: &mut Frame,
    conn: &ConnectionState,
    focused: FocusedPanel,
    area: Rect,
) {
    let is_focused = focused == FocusedPanel::Connections;

    let conn_items: Vec<ListItem> = conn
        .connections
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let is_active = conn.active_id == Some(c.id);
            let indicator = if is_active { "● " } else { "  " };
            let style = if i == conn.selected && is_focused {
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
        " Connections (Enter=connect  n=new  e=edit  d=delete) "
    } else if conn.connections.is_empty() {
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
    conn_state.select(Some(conn.selected));
    frame.render_stateful_widget(conn_list, area, &mut conn_state);
}

// ---------------------------------------------------------------------------
// Tables panel (bottom-left)
// ---------------------------------------------------------------------------

pub fn draw_tables(
    frame: &mut Frame,
    tables: &TableBrowserState,
    focused: FocusedPanel,
    is_loading: bool,
    area: Rect,
) {
    let is_focused = focused == FocusedPanel::Tables;

    let border_style = if is_focused {
        Style::default().fg(theme::BLUE)
    } else {
        Style::default().fg(theme::OVERLAY0)
    };

    let table_items: Vec<ListItem> = tables
        .tables
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let style = if i == tables.selected && is_focused {
                Style::default()
                    .fg(theme::BASE)
                    .bg(theme::YELLOW)
                    .add_modifier(Modifier::BOLD)
            } else if i == tables.selected {
                Style::default().fg(theme::YELLOW)
            } else {
                Style::default().fg(theme::OVERLAY2)
            };
            ListItem::new(Span::styled(t.qualified(), style))
        })
        .collect();

    let table_title = if is_loading && tables.tables.is_empty() {
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
    tbl_state.select(if tables.tables.is_empty() {
        None
    } else {
        Some(tables.selected)
    });
    frame.render_stateful_widget(table_list, area, &mut tbl_state);
}

// ---------------------------------------------------------------------------
// Connection form overlay
// ---------------------------------------------------------------------------

pub fn draw_form(frame: &mut Frame, form: &ConnectionForm, screen: Rect) {
    let area = centered_rect(60, 70, screen);

    frame.render_widget(Clear, area);

    let title = if form.editing_id.is_some() {
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

    let field_count = form.field_count();
    let mut constraints = vec![Constraint::Length(3); field_count];
    constraints.push(Constraint::Min(1)); // spacer
    constraints.push(Constraint::Length(1)); // help line

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for i in 0..field_count {
        let label = form.field_label(i);
        let is_active = form.field_index == i;
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

        // Backend field (field 0 for all backends)
        if i == 0 {
            let backend_display = match form.backend {
                sbql_core::DbBackend::Postgres => "PostgreSQL",
                sbql_core::DbBackend::Sqlite => "SQLite",
                sbql_core::DbBackend::Redis => "Redis",
            };
            let hint = if is_active { "  Space: cycle" } else { "" };
            let para = Paragraph::new(format!("{backend_display}{hint}")).block(
                Block::default()
                    .title(Span::styled(format!(" {label} "), title_style))
                    .borders(Borders::ALL)
                    .border_style(border_style),
            );
            frame.render_widget(para, chunks[i]);
            continue;
        }

        // SSL Mode field (only PG, last field)
        if form.backend == sbql_core::DbBackend::Postgres && i == 7 {
            let ssl_display = form.ssl_mode.as_str().to_owned();
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

        // Text fields
        let value = match form.backend {
            sbql_core::DbBackend::Postgres => match i {
                1 => &form.name,
                2 => &form.host,
                3 => &form.port,
                4 => &form.user,
                5 => &form.database,
                6 => &form.password,
                _ => continue,
            },
            sbql_core::DbBackend::Sqlite => match i {
                1 => &form.name,
                2 => &form.file_path,
                _ => continue,
            },
            sbql_core::DbBackend::Redis => match i {
                1 => &form.name,
                2 => &form.host,
                3 => &form.port,
                4 => &form.password,
                5 => &form.database,
                _ => continue,
            },
        };

        let is_password = (form.backend == sbql_core::DbBackend::Postgres && i == 6)
            || (form.backend == sbql_core::DbBackend::Redis && i == 4);
        let display = if is_password {
            if value.is_empty() && form.editing_id.is_some() {
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

    let help = Paragraph::new("Tab/↑↓: next field  Space: cycle  Enter: save  Esc: cancel")
        .style(Style::default().fg(theme::OVERLAY0));
    frame.render_widget(help, *chunks.last().unwrap());

    if let Some(ref err) = form.error {
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
