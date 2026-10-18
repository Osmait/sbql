//! Left panels — connection list, table list, and add/edit form overlay.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::{
    ConnectionEntry, ConnectionForm, ConnectionState, FocusedPanel, TableBrowserState,
};
use crate::ui::hit::{HitMap, Zone};
use crate::ui::theme;

/// Screen row of list item `i`, or `None` once the list runs past the panel.
///
/// These lists do not scroll, so item `i` is simply the `i`-th row inside the
/// border. Registering rows that fall outside would hand clicks to items the
/// user cannot see.
fn row_y(inner: Rect, i: usize) -> Option<u16> {
    let y = inner.y.checked_add(u16::try_from(i).ok()?)?;
    (y < inner.y + inner.height).then_some(y)
}

/// How wide a string paints, measured the way ratatui measures it.
pub(super) fn width_of(s: &str) -> u16 {
    u16::try_from(Span::raw(s).width()).unwrap_or(u16::MAX)
}

/// The part of a one-row `line` holding the columns `[start, start + width)`.
///
/// Both overlays turn a word inside a single painted row into a button, and
/// both rows get truncated when the terminal is narrow. Clipping to the row
/// leaves a fully cut-off word zero-width, which `HitMap::register` drops —
/// so a word that was never drawn is never clickable either.
pub(super) fn label_rect(line: Rect, start: u16, width: u16) -> Rect {
    let x = line.x.saturating_add(start).min(line.right());
    let end = x.saturating_add(width).min(line.right());
    Rect {
        x,
        y: line.y,
        width: end - x,
        height: line.height,
    }
}

// ---------------------------------------------------------------------------
// Connections panel (top-left)
// ---------------------------------------------------------------------------

pub(crate) fn draw_connections(
    frame: &mut Frame,
    conn: &ConnectionState,
    focused: FocusedPanel,
    area: Rect,
    hits: &mut HitMap,
) {
    let is_focused = focused == FocusedPanel::Connections;

    // One zone per drawn row, so a click resolves to a row without the mouse
    // handler re-deriving the list's geometry from the panel rect.
    let inner = area.inner(ratatui::layout::Margin::new(1, 1));
    for (i, _) in conn.entries().enumerate() {
        let Some(y) = row_y(inner, i) else { break };
        hits.register(
            Rect::new(inner.x, y, inner.width, 1),
            Zone::ConnectionRow(i),
        );
    }

    let conn_items: Vec<ListItem> = conn
        .entries()
        .enumerate()
        .map(|(i, entry)| {
            let c = entry.config();
            let is_active = conn.active_id == Some(c.id);
            let indicator = if is_active { "● " } else { "  " };
            let style = if i == conn.selected() && is_focused {
                Style::default()
                    .fg(theme::base())
                    .bg(theme::blue())
                    .add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default().fg(theme::green())
            } else if entry.is_discovered() {
                // Dimmed so the list reads at a glance as "yours" and "found
                // for you" — these vanish when the container stops.
                Style::default().fg(theme::overlay1())
            } else {
                Style::default()
            };

            let mut spans = vec![
                Span::styled(
                    indicator,
                    Style::default().fg(if is_active {
                        theme::green()
                    } else {
                        theme::overlay0()
                    }),
                ),
                Span::styled(c.name.clone(), style),
            ];
            if let ConnectionEntry::Discovered(found) = entry {
                spans.push(Span::styled(
                    format!("  {}", found.source.label()),
                    Style::default().fg(theme::overlay0()),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let has_discovered = !conn.discovered.is_empty();
    let conn_title = if is_focused && has_discovered {
        " Connections (Enter=connect  n=new  e=edit  d=delete  s=save docker one) "
    } else if is_focused {
        " Connections (Enter=connect  n=new  e=edit  d=delete) "
    } else if conn.is_empty() {
        " Connections (n=new) "
    } else {
        " Connections "
    };

    let border_style = if is_focused {
        Style::default().fg(theme::blue())
    } else {
        Style::default().fg(theme::overlay0())
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
    conn_state.select(Some(conn.selected()));
    frame.render_stateful_widget(conn_list, area, &mut conn_state);
}

// ---------------------------------------------------------------------------
// Tables panel (bottom-left)
// ---------------------------------------------------------------------------

pub(crate) fn draw_tables(
    frame: &mut Frame,
    tables: &TableBrowserState,
    focused: FocusedPanel,
    is_loading: bool,
    area: Rect,
    hits: &mut HitMap,
) {
    let is_focused = focused == FocusedPanel::Tables;

    let inner = area.inner(ratatui::layout::Margin::new(1, 1));
    for i in 0..tables.tables.len() {
        let Some(y) = row_y(inner, i) else { break };
        hits.register(Rect::new(inner.x, y, inner.width, 1), Zone::TableRow(i));
    }

    let border_style = if is_focused {
        Style::default().fg(theme::blue())
    } else {
        Style::default().fg(theme::overlay0())
    };

    let table_items: Vec<ListItem> = tables
        .tables
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let style = if i == tables.selected() && is_focused {
                Style::default()
                    .fg(theme::base())
                    .bg(theme::yellow())
                    .add_modifier(Modifier::BOLD)
            } else if i == tables.selected() {
                Style::default().fg(theme::yellow())
            } else {
                Style::default().fg(theme::overlay2())
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
        Some(tables.selected())
    });
    frame.render_stateful_widget(table_list, area, &mut tbl_state);
}

// ---------------------------------------------------------------------------
// Connection form overlay
// ---------------------------------------------------------------------------

/// The help line, in the pieces the mouse cares about. "Enter" and "Esc" are
/// the only save/cancel affordances the dialog paints, so those two words are
/// its buttons — kept apart from the text around them so the hit rects are
/// measured from the strings that get painted rather than from counted columns.
const HELP_LEAD: &str = "Tab/↑↓: next field  Space: cycle  ";
const HELP_SAVE: &str = "Enter: save";
const HELP_GAP: &str = "  ";
const HELP_CANCEL: &str = "Esc: cancel";

/// Rows the dialog spends on anything but fields: two borders, the line an
/// error is written on, and the help line.
const FORM_CHROME: u16 = 4;

pub(crate) fn draw_form(frame: &mut Frame, form: &ConnectionForm, screen: Rect, hits: &mut HitMap) {
    let field_count = form.field_count();

    // Sized to what it holds, not to a share of the screen. 70% of an 80x24
    // terminal is 16 rows; the Postgres form needs 28, so every box was
    // squeezed down to its own border and no value was visible at all. When
    // the bordered boxes do not fit, each field gets one row instead, and if
    // even that is too tall the list scrolls to keep the active field shown.
    let rows_for = |per_field: usize| {
        u16::try_from(field_count * per_field)
            .unwrap_or(u16::MAX)
            .saturating_add(FORM_CHROME)
    };
    let compact = rows_for(3) > screen.height;
    let row_height: u16 = if compact { 1 } else { 3 };
    let height = rows_for(usize::from(row_height)).min(screen.height);
    let area = centered_rect(60, height, screen);

    frame.render_widget(Clear, area);

    let title = if form.draft.id.is_some() {
        " Edit Connection "
    } else {
        " New Connection "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::blue()));

    frame.render_widget(block.clone(), area);
    let inner = block.inner(area);

    let room = usize::from(inner.height.saturating_sub(FORM_CHROME - 2) / row_height);
    let visible = field_count.min(room);
    // The first row shown: enough to keep the active field on the last line.
    let first = form
        .field_index
        .saturating_sub(visible.saturating_sub(1))
        .min(field_count - visible);

    let mut constraints = vec![Constraint::Length(row_height); visible];
    constraints.push(Constraint::Min(1)); // spacer, where an error goes
    constraints.push(Constraint::Length(1)); // help line

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    let label_width = (0..field_count)
        .map(|i| width_of(form.field_label(i)))
        .max()
        .unwrap_or(0);

    for (slot, i) in (first..first + visible).enumerate() {
        let label = form.field_label(i);
        let is_active = form.field_index == i;
        let border_style = if is_active {
            Style::default().fg(theme::blue())
        } else {
            Style::default().fg(theme::overlay0())
        };
        let title_style = if is_active {
            Style::default()
                .fg(theme::blue())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::overlay2())
        };

        // One match on the row rather than two lookups and an `expect` to
        // reconcile them: the compiler now proves the text-field case has a
        // spec, instead of a comment claiming it.
        let display = match form.field_at(i) {
            // Row 0, the backend picker.
            None => {
                let hint = if is_active { "  Space: cycle" } else { "" };
                format!("{}{hint}", form.draft.backend.label())
            }
            // A choice field the backend declares, e.g. SSL Mode.
            Some(spec) if !spec.field.is_text() => {
                let hint = if is_active { "  Space: cycle" } else { "" };
                format!("{}{hint}", form.draft.value(spec.field))
            }
            // A field the user types into. Secrets are starred, and a stored
            // password left untouched reads as unchanged rather than as empty.
            Some(spec) => {
                let value = form.draft.value(spec.field);
                if spec.field.is_secret() {
                    if value.is_empty() && form.draft.id.is_some() {
                        "(unchanged)".to_owned()
                    } else {
                        "*".repeat(value.len())
                    }
                } else {
                    value.to_owned()
                }
            }
        };

        // `chunks` has one entry per shown field plus two, but a panic here
        // would take down the whole app mid-frame. Skipping a row is survivable.
        let Some(&row) = chunks.get(slot) else {
            continue;
        };

        // Registered inside the loop that paints the row, so a backend with
        // fewer fields leaves no zone behind for the rows it stopped drawing.
        // The whole bordered box counts: its border is part of the field.
        hits.register(row, Zone::FormField(i));

        let para = if compact {
            let marker = if is_active { "▸ " } else { "  " };
            let pad = " ".repeat(usize::from(label_width.saturating_sub(width_of(label))));
            Paragraph::new(Line::from(vec![
                Span::styled(format!("{marker}{label}{pad}  "), title_style),
                Span::styled(display, border_style),
            ]))
        } else {
            Paragraph::new(display).block(
                Block::default()
                    .title(Span::styled(format!(" {label} "), title_style))
                    .borders(Borders::ALL)
                    .border_style(border_style),
            )
        };
        frame.render_widget(para, row);
    }

    if let Some(&help_area) = chunks.last() {
        let save_x = width_of(HELP_LEAD);
        let cancel_x = save_x
            .saturating_add(width_of(HELP_SAVE))
            .saturating_add(width_of(HELP_GAP));
        // A narrow dialog truncates the line, so both words are clipped to what
        // the row had room for; a fully clipped one registers nothing.
        hits.register(
            label_rect(help_area, save_x, width_of(HELP_SAVE)),
            Zone::FormSubmit,
        );
        hits.register(
            label_rect(help_area, cancel_x, width_of(HELP_CANCEL)),
            Zone::FormCancel,
        );

        let help = Paragraph::new(format!("{HELP_LEAD}{HELP_SAVE}{HELP_GAP}{HELP_CANCEL}"))
            .style(Style::default().fg(theme::overlay0()));
        frame.render_widget(help, help_area);
    }

    if let Some(ref err) = form.error {
        let err_area = Rect {
            y: inner.y + inner.height.saturating_sub(2),
            height: 1,
            ..inner
        };
        frame.render_widget(
            Paragraph::new(err.as_str()).style(Style::default().fg(theme::red())),
            err_area,
        );
    }
}

// ---------------------------------------------------------------------------
// Helper: centered rect
// ---------------------------------------------------------------------------

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let row = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(height)])
        .flex(ratatui::layout::Flex::Center)
        .split(r)[0];

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(row)[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};
    use sbql_core::DbBackend;

    /// The text painted inside a rect, so a test can check a zone against the
    /// pixels it claims rather than against coordinates it worked out itself.
    fn text_at(buf: &Buffer, rect: Rect) -> String {
        let mut out = String::new();
        for y in rect.y..rect.bottom() {
            for x in rect.x..rect.right() {
                if let Some(cell) = buf.cell((x, y)) {
                    out.push_str(cell.symbol());
                }
            }
        }
        out
    }

    /// Where a zone ended up, found by asking the map rather than by redoing
    /// the layout arithmetic the draw already did.
    fn zone_rect(hits: &HitMap, screen: Rect, want: Zone) -> Option<Rect> {
        (0..screen.height).find_map(|y| {
            (0..screen.width).find_map(|x| match hits.hit(x, y) {
                Some((rect, zone)) if zone == want => Some(rect),
                _ => None,
            })
        })
    }

    fn render(form: &ConnectionForm, width: u16, height: u16) -> (Buffer, HitMap) {
        let mut hits = HitMap::default();
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("backend");
        terminal
            .draw(|frame| draw_form(frame, form, frame.area(), &mut hits))
            .expect("draw");
        (terminal.backend().buffer().clone(), hits)
    }

    /// A click on a field box focuses that field, so the zone's index has to be
    /// the index of the field whose label is painted in it — including the last
    /// one, which is the row a wrong bound would drop.
    #[test]
    fn every_field_row_answers_with_its_own_index() {
        let screen = Rect::new(0, 0, 100, 44);
        let form = ConnectionForm::open_new(); // Postgres: the longest field list
        let (buffer, hits) = render(&form, screen.width, screen.height);

        for i in 0..form.field_count() {
            let row = zone_rect(&hits, screen, Zone::FormField(i));
            assert!(row.is_some(), "no zone for row {i}");
            if let Some(rect) = row {
                assert!(
                    text_at(&buffer, rect).contains(form.field_label(i)),
                    "row {i} claims a box that does not hold its label"
                );
            }
        }

        let last = form.field_count() - 1;
        let rect = zone_rect(&hits, screen, Zone::FormField(last)).expect("the last row");
        assert_eq!(
            hits.hit(rect.x + 1, rect.y + 1).map(|(_, z)| z),
            Some(Zone::FormField(last))
        );
        assert!(text_at(&buffer, rect).contains("SSL Mode"));
    }

    /// SQLite needs three rows where Postgres needs eight. The space the longer
    /// form used is empty now, and empty space must not focus a field.
    #[test]
    fn a_shorter_backend_leaves_no_zone_where_it_stopped_drawing() {
        let screen = Rect::new(0, 0, 100, 44);

        let postgres = ConnectionForm::open_new();
        let (_, pg_hits) = render(&postgres, screen.width, screen.height);
        let dropped = zone_rect(
            &pg_hits,
            screen,
            Zone::FormField(postgres.field_count() - 1),
        )
        .expect("Postgres draws a last row");

        let mut form = ConnectionForm::open_new();
        form.draft.set_backend(DbBackend::Sqlite);
        assert_eq!(form.field_count(), 3);
        let (_, hits) = render(&form, screen.width, screen.height);

        assert!(
            zone_rect(&hits, screen, Zone::FormField(3)).is_none(),
            "a row SQLite never draws is clickable"
        );
        assert_eq!(
            hits.hit(dropped.x + 1, dropped.y + 1),
            None,
            "the space the longer form used still answers for a field"
        );
    }

    /// "Enter" and "Esc" in the help line are the dialog's only save/cancel
    /// affordances, so their zones have to sit exactly on those words.
    #[test]
    fn the_help_line_words_are_the_save_and_cancel_buttons() {
        let screen = Rect::new(0, 0, 100, 44);
        let (buffer, hits) = render(&ConnectionForm::open_new(), screen.width, screen.height);

        let save = zone_rect(&hits, screen, Zone::FormSubmit).expect("a save zone");
        let cancel = zone_rect(&hits, screen, Zone::FormCancel).expect("a cancel zone");

        assert_eq!(text_at(&buffer, save), HELP_SAVE);
        assert_eq!(text_at(&buffer, cancel), HELP_CANCEL);
    }

    /// A narrow terminal cuts the help line short. What was never painted must
    /// not stay clickable, or the dialog grows an invisible Save button.
    #[test]
    fn a_truncated_help_line_drops_the_words_it_could_not_paint() {
        let screen = Rect::new(0, 0, 60, 44);
        let (buffer, hits) = render(&ConnectionForm::open_new(), screen.width, screen.height);

        let painted = text_at(&buffer, screen);
        assert!(!painted.contains(HELP_SAVE), "the premise changed");
        assert!(zone_rect(&hits, screen, Zone::FormSubmit).is_none());
        assert!(zone_rect(&hits, screen, Zone::FormCancel).is_none());
    }

    /// 80x24 is the terminal everyone falls back to. The Postgres form needs
    /// more than that boxed, and used to squeeze every box down to its border
    /// with no value showing; now each field gets a line and keeps its value.
    #[test]
    fn the_postgres_form_is_readable_in_80_by_24() {
        let screen = Rect::new(0, 0, 80, 24);
        let mut form = ConnectionForm::open_new();
        form.draft.host = "db.example.com".into();
        let (buffer, hits) = render(&form, screen.width, screen.height);

        for i in 0..form.field_count() {
            let rect = zone_rect(&hits, screen, Zone::FormField(i))
                .unwrap_or_else(|| panic!("row {i} is not drawn"));
            assert!(text_at(&buffer, rect).contains(form.field_label(i)));
        }
        assert!(text_at(&buffer, screen).contains("PostgreSQL"));
        assert!(text_at(&buffer, screen).contains("db.example.com"));
    }

    /// Shorter still, the list scrolls rather than dropping the field the
    /// user is typing into.
    #[test]
    fn a_tiny_terminal_keeps_the_active_field_in_view() {
        let screen = Rect::new(0, 0, 80, 10);
        let mut form = ConnectionForm::open_new();
        form.field_index = form.field_count() - 1;
        let (buffer, hits) = render(&form, screen.width, screen.height);

        let last = form.field_count() - 1;
        let rect = zone_rect(&hits, screen, Zone::FormField(last)).expect("the active row");
        assert!(text_at(&buffer, rect).contains("SSL Mode"));
        assert!(zone_rect(&hits, screen, Zone::FormField(0)).is_none());
    }
}
