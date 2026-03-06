use std::io;

use sbql_core::load_connections;
use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEvent, KeyModifiers,
        KeyboardEnhancementFlags, MouseButton, MouseEvent, MouseEventKind,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, supports_keyboard_enhancement, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use ratatui::{backend::CrosstermBackend, layout::Rect, Terminal};
use sbql_core::CoreCommand;
use tokio::sync::mpsc;
use tui_textarea::Input;

mod app;
mod events;
mod ui;
mod worker;

use app::{AppState, EditorMode, FocusedPanel, NavMode};
use tui_textarea::CursorMove;
use events::{is_commit, is_quit, is_run_query, spawn_event_reader, AppEvent};
use worker::spawn_worker;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ---------------------------------------------------------------------------
    // CLI: optional connection name  —  sbql [connection-name]
    // ---------------------------------------------------------------------------
    let auto_connect_name: Option<String> = std::env::args().nth(1).map(|s| s.trim().to_owned());

    // Validate the name before entering raw mode so errors print cleanly.
    if let Some(ref name) = auto_connect_name {
        let saved = load_connections().unwrap_or_default();
        let found = saved.iter().any(|c| c.name.eq_ignore_ascii_case(name));
        if !found {
            let names: Vec<&str> = saved.iter().map(|c| c.name.as_str()).collect();
            if names.is_empty() {
                eprintln!("sbql: no saved connections found. Add one with `sbql` (no arguments) and press `n`.");
            } else {
                eprintln!("sbql: connection '{}' not found.", name);
                eprintln!("Available connections:");
                for n in &names {
                    eprintln!("  {}", n);
                }
            }
            std::process::exit(1);
        }
    }

    // Tracing to a file to avoid corrupting the TUI output
    let log_file = std::fs::File::create("/tmp/sbql.log")?;
    let log_file = std::io::LineWriter::new(log_file);
    tracing_subscriber::fmt()
        .with_writer(std::sync::Mutex::new(log_file))
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("sbql_core=info".parse()?)
                .add_directive("sbql_tui=info".parse()?),
        )
        .init();

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    // Enable Kitty keyboard protocol if the terminal supports it.
    // This allows crossterm to correctly distinguish Ctrl+Enter from Enter,
    // Alt+hjkl from plain hjkl, and other ambiguous modifier combinations.
    let keyboard_enhanced = supports_keyboard_enhancement().unwrap_or(false);
    if keyboard_enhanced {
        execute!(
            stdout,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES,
            )
        )?;
    }

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // App state, worker, and event reader
    let (cmd_tx, mut event_rx) = spawn_worker();
    let (app_tx, mut app_rx) = mpsc::unbounded_channel::<AppEvent>();
    spawn_event_reader(app_tx.clone());

    // Forward Core events into the unified AppEvent channel
    {
        let app_tx2 = app_tx.clone();
        tokio::spawn(async move {
            while let Some(ev) = event_rx.recv().await {
                if app_tx2.send(AppEvent::Core(ev)).is_err() {
                    break;
                }
            }
        });
    }

    // 100ms ticker — drives spinner animation
    {
        let app_tx3 = app_tx.clone();
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(std::time::Duration::from_millis(100));
            loop {
                interval.tick().await;
                if app_tx3.send(AppEvent::Tick).is_err() {
                    break;
                }
            }
        });
    }

    // Initial connection list comes via the worker startup event.
    // `auto_connected` tracks whether we already fired the auto-connect so we
    // don't fire it again on subsequent ConnectionList refreshes.
    let mut state = AppState::new(Vec::new());
    let mut auto_connected = false;
    // Initial draw
    terminal.draw(|f| ui::draw(f, &mut state))?;

    loop {
        // Block until the next event
        let event = match app_rx.recv().await {
            Some(e) => e,
            None => break,
        };

        match event {
            AppEvent::Core(ce) => {
                tracing::debug!("CoreEvent: {:?}", ce);
                // Auto-request table list immediately after a successful connect
                let auto_list = matches!(ce, sbql_core::CoreEvent::Connected(_));
                // If a connection name was supplied on the CLI and the connection
                // list just arrived for the first time, fire the connect command.
                if !auto_connected {
                    if let sbql_core::CoreEvent::ConnectionList(ref conns) = ce {
                        if let Some(ref name) = auto_connect_name {
                            if let Some(cfg) = conns
                                .iter()
                                .find(|c| c.name.eq_ignore_ascii_case(name))
                            {
                                let _ = cmd_tx.send(CoreCommand::Connect(cfg.id));
                                auto_connected = true;
                            }
                        }
                    }
                }
                state.apply_core_event(ce);
                if auto_list {
                    let _ = cmd_tx.send(CoreCommand::ListTables);
                }
            }

            AppEvent::Resize(_, _) => {}

            AppEvent::IoError(e) => {
                state.error_msg = Some(format!("IO error: {e}"));
            }

            AppEvent::Key(key) => {
                handle_key(&mut state, key, &cmd_tx);
                if state.should_quit {
                    break;
                }
            }

            AppEvent::Mouse(mouse) => {
                handle_mouse(&mut state, mouse, &cmd_tx);
            }

            AppEvent::Tick => {
                if state.is_loading {
                    state.spinner_frame = state.spinner_frame.wrapping_add(1);
                }
            }
        }

        // Redraw after every event (reactive model)
        terminal.draw(|f| ui::draw(f, &mut state))?;
    }

    // Restore terminal
    disable_raw_mode()?;
    if keyboard_enhanced {
        execute!(
            terminal.backend_mut(),
            PopKeyboardEnhancementFlags
        )?;
    }
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Keyboard handler
// ---------------------------------------------------------------------------

fn handle_key(
    state: &mut AppState,
    key: KeyEvent,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    tracing::info!(
        "handle_key: focused={:?} code={:?} mods={:?} cell_edit={} conn_form={} filter={}",
        state.focused, key.code, key.modifiers,
        state.cell_edit.is_some(),
        state.conn_form.visible,
        state.filter_bar.visible,
    );

    // ---- Diagram mode — intercept all keys ----
    if state.diagram.is_some() {
        handle_key_diagram(state, key);
        return;
    }

    // ---- Cell edit mode ----
    if state.cell_edit.is_some() {
        handle_key_cell_edit(state, key, cmd_tx);
        return;
    }

    // ---- Filter bar mode ----
    if state.filter_bar.visible {
        handle_key_filter(state, key, cmd_tx);
        return;
    }

    // ---- Connection form mode ----
    if state.conn_form.visible {
        handle_key_conn_form(state, key, cmd_tx);
        return;
    }

    // ---- Pending destructive confirmation ----
    if state.pending_connection_delete.is_some() {
        handle_key_confirm_delete_connection(state, key, cmd_tx);
        return;
    }

    // ---- Global keys ----
    if is_quit(&key) {
        state.should_quit = true;
        return;
    }

    // Ctrl+\ — toggle sidebar visibility (fallback shortcut)
    if key.code == KeyCode::Char('\\') && key.modifiers == KeyModifiers::CONTROL {
        toggle_sidebar(state);
        return;
    }

    // In Editor Insert mode, keep typing local to editor.
    if state.editor_mode == EditorMode::Insert && state.focused == FocusedPanel::Editor {
        if key.code == KeyCode::Esc {
            state.editor_mode = EditorMode::Normal;
            state.nav_mode = NavMode::Panel;
            state.pending_leader = false;
            return;
        }
        handle_key_editor(state, key, cmd_tx);
        return;
    }

    // Esc leaves panel mode and returns to global mode.
    if key.code == KeyCode::Esc {
        state.pending_leader = false;
        state.editor_mode = EditorMode::Normal;
        if state.nav_mode == NavMode::Panel {
            state.nav_mode = NavMode::Global;
            state.status_msg = Some("Global mode".into());
            state.error_msg = None;
        }
        return;
    }

    // Reliable panel shortcuts (helpful on macOS when Alt/Option is not sent as Meta)
    // F1/F2/F3/F4 or Ctrl+1/Ctrl+2/Ctrl+3/Ctrl+4
    let focus_target = match (key.code, key.modifiers) {
        (KeyCode::F(1), _) | (KeyCode::Char('1'), KeyModifiers::CONTROL) => {
            Some(FocusedPanel::Connections)
        }
        (KeyCode::F(2), _) | (KeyCode::Char('2'), KeyModifiers::CONTROL) => {
            Some(FocusedPanel::Tables)
        }
        (KeyCode::F(3), _) | (KeyCode::Char('3'), KeyModifiers::CONTROL) => {
            Some(FocusedPanel::Editor)
        }
        (KeyCode::F(4), _) | (KeyCode::Char('4'), KeyModifiers::CONTROL) => {
            Some(FocusedPanel::Results)
        }
        _ => None,
    };
    if let Some(target) = focus_target {
        state.focused = if state.sidebar_hidden
            && (target == FocusedPanel::Connections || target == FocusedPanel::Tables)
        {
            FocusedPanel::Editor
        } else {
            target
        };
        return;
    }

    // Shift+D = open database diagram
    if key.code == KeyCode::Char('D') {
        if state.active_connection_id.is_some() {
            let _ = cmd_tx.send(CoreCommand::LoadDiagram);
        } else {
            state.error_msg = Some("Connect to a database first (Enter on a connection).".into());
        }
        return;
    }

    // Tab / BackTab cycles focus through the 4 panels in order:
    //   Connections → Tables → Editor → Results → Connections …
    // When the sidebar is hidden, Connections and Tables are skipped.
    if key.code == KeyCode::Tab && key.modifiers == KeyModifiers::NONE {
        state.focused = tab_next(state.focused, state.sidebar_hidden);
        return;
    }
    if key.code == KeyCode::BackTab {
        state.focused = tab_prev(state.focused, state.sidebar_hidden);
        return;
    }

    if state.nav_mode == NavMode::Global {
        if state.pending_leader {
            state.pending_leader = false;
            match (key.code, key.modifiers) {
                (KeyCode::Char('e'), KeyModifiers::NONE) => toggle_sidebar(state),
                _ => state.status_msg = Some("Unknown leader combo. Try: Space e".into()),
            }
            return;
        }

        if key.code == KeyCode::Char(' ') && key.modifiers == KeyModifiers::NONE {
            state.pending_leader = true;
            state.status_msg = Some("Leader: _  (e: toggle sidebar)".into());
            state.error_msg = None;
            return;
        }

        if key.code == KeyCode::Char('i') && key.modifiers == KeyModifiers::NONE {
            match state.focused {
                FocusedPanel::Results => {
                    state.nav_mode = NavMode::Panel;
                    enter_cell_edit_mode(state, cmd_tx);
                }
                FocusedPanel::Editor => {
                    state.nav_mode = NavMode::Panel;
                    state.editor_mode = EditorMode::Insert;
                }
                FocusedPanel::Connections | FocusedPanel::Tables => {
                    state.focused = FocusedPanel::Editor;
                    state.nav_mode = NavMode::Panel;
                    state.editor_mode = EditorMode::Insert;
                }
            }
            return;
        }

        if key.code == KeyCode::Enter {
            state.nav_mode = NavMode::Panel;
            state.status_msg = Some("Panel mode".into());
            state.error_msg = None;
            return;
        }

        if try_navigate_panels(state, key) {
            return;
        }

        return;
    }

    // In panel mode, Alt+hjkl still navigates panel focus.
    if key.modifiers == KeyModifiers::ALT && try_navigate_panels(state, key) {
        return;
    }

    // In panel mode, `i` inside editor enters Insert mode.
    if state.focused == FocusedPanel::Editor
        && key.code == KeyCode::Char('i')
        && key.modifiers == KeyModifiers::NONE
        && state.editor_mode == EditorMode::Normal
    {
        state.editor_mode = EditorMode::Insert;
        return;
    }

    match state.focused {
        FocusedPanel::Connections => handle_key_connections(state, key, cmd_tx),
        FocusedPanel::Tables      => handle_key_tables(state, key, cmd_tx),
        FocusedPanel::Editor      => handle_key_editor(state, key, cmd_tx),
        FocusedPanel::Results     => handle_key_results(state, key, cmd_tx),
    }
}

fn handle_key_confirm_delete_connection(
    state: &mut AppState,
    key: KeyEvent,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
            if let Some((id, name)) = state.pending_connection_delete.take() {
                let _ = cmd_tx.send(CoreCommand::DeleteConnection(id));
                state.status_msg = Some(format!("Deleted connection '{name}'."));
                state.error_msg = None;
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            state.pending_connection_delete = None;
            state.status_msg = Some("Delete cancelled.".into());
            state.error_msg = None;
        }
        _ => {}
    }
}

fn try_navigate_panels(state: &mut AppState, key: KeyEvent) -> bool {
    let sidebar = !state.sidebar_hidden;
    match key.code {
        // l — left column → right column
        KeyCode::Char('l') | KeyCode::Right => {
            match state.focused {
                FocusedPanel::Connections => state.focused = FocusedPanel::Editor,
                FocusedPanel::Tables => state.focused = FocusedPanel::Results,
                _ => {}
            }
            true
        }
        // h — right column → left column
        KeyCode::Char('h') | KeyCode::Left => {
            if sidebar {
                match state.focused {
                    FocusedPanel::Editor => state.focused = FocusedPanel::Connections,
                    FocusedPanel::Results => state.focused = FocusedPanel::Tables,
                    _ => {}
                }
            }
            true
        }
        // j — move down within same column
        KeyCode::Char('j') | KeyCode::Down => {
            match state.focused {
                FocusedPanel::Connections => state.focused = FocusedPanel::Tables,
                FocusedPanel::Editor => state.focused = FocusedPanel::Results,
                _ => {}
            }
            true
        }
        // k — move up within same column
        KeyCode::Char('k') | KeyCode::Up => {
            match state.focused {
                FocusedPanel::Tables => state.focused = FocusedPanel::Connections,
                FocusedPanel::Results => state.focused = FocusedPanel::Editor,
                _ => {}
            }
            true
        }
        _ => false,
    }
}

/// Cycle forward through panels: Connections → Tables → Editor → Results → …
fn tab_next(current: FocusedPanel, sidebar_hidden: bool) -> FocusedPanel {
    match current {
        FocusedPanel::Connections => {
            if sidebar_hidden { FocusedPanel::Editor } else { FocusedPanel::Tables }
        }
        FocusedPanel::Tables  => FocusedPanel::Editor,
        FocusedPanel::Editor  => FocusedPanel::Results,
        FocusedPanel::Results => {
            if sidebar_hidden { FocusedPanel::Editor } else { FocusedPanel::Connections }
        }
    }
}

/// Cycle backward through panels.
fn tab_prev(current: FocusedPanel, sidebar_hidden: bool) -> FocusedPanel {
    match current {
        FocusedPanel::Connections => {
            if sidebar_hidden { FocusedPanel::Results } else { FocusedPanel::Results }
        }
        FocusedPanel::Tables  => FocusedPanel::Connections,
        FocusedPanel::Editor  => {
            if sidebar_hidden { FocusedPanel::Results } else { FocusedPanel::Tables }
        }
        FocusedPanel::Results => FocusedPanel::Editor,
    }
}

fn toggle_sidebar(state: &mut AppState) {
    state.sidebar_hidden = !state.sidebar_hidden;
    if state.sidebar_hidden
        && (state.focused == FocusedPanel::Connections || state.focused == FocusedPanel::Tables)
    {
        state.focused = FocusedPanel::Editor;
    }
    state.status_msg = Some(if state.sidebar_hidden {
        "Sidebar hidden".into()
    } else {
        "Sidebar shown".into()
    });
    state.error_msg = None;
}

// ---- Connections panel keys ----
fn handle_key_connections(
    state: &mut AppState,
    key: KeyEvent,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            state.pending_g = false;
            if !state.connections.is_empty() {
                let next = state.selected_connection + 1;
                if next < state.connections.len() {
                    state.selected_connection = next;
                }
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.pending_g = false;
            state.selected_connection = state.selected_connection.saturating_sub(1);
        }
        // G = last connection
        KeyCode::Char('G') => {
            state.pending_g = false;
            if !state.connections.is_empty() {
                state.selected_connection = state.connections.len() - 1;
            }
        }
        // gg = first connection
        KeyCode::Char('g') => {
            if state.pending_g {
                state.selected_connection = 0;
                state.pending_g = false;
            } else {
                state.pending_g = true;
            }
        }
        KeyCode::Enter => {
            state.pending_g = false;
            if let Some(cfg) = state.connections.get(state.selected_connection) {
                let id = cfg.id;
                let _ = cmd_tx.send(CoreCommand::Connect(id));
            }
        }
        KeyCode::Char('n') => {
            state.pending_g = false;
            state.conn_form = app::ConnectionForm::open_new();
        }
        KeyCode::Char('e') => {
            state.pending_g = false;
            if let Some(cfg) = state.connections.get(state.selected_connection).cloned() {
                state.conn_form = app::ConnectionForm::open_edit(&cfg);
            }
        }
        KeyCode::Char('d') => {
            state.pending_g = false;
            if let Some(cfg) = state.connections.get(state.selected_connection).cloned() {
                state.pending_connection_delete = Some((cfg.id, cfg.name.clone()));
                state.status_msg = Some(format!(
                    "Confirm delete connection '{}': y/Enter = confirm, n/Esc = cancel.",
                    cfg.name
                ));
                state.error_msg = None;
            }
        }
        KeyCode::Char('x') => {
            state.pending_g = false;
            if let Some(id) = state.active_connection_id {
                let _ = cmd_tx.send(CoreCommand::Disconnect(id));
            }
        }
        KeyCode::Esc => {
            state.pending_g = false;
            state.focused = FocusedPanel::Editor;
        }
        _ => { state.pending_g = false; }
    }
}

// ---- Tables panel keys ----
fn handle_key_tables(
    state: &mut AppState,
    key: KeyEvent,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            state.pending_g = false;
            if !state.tables.is_empty() {
                state.selected_table =
                    (state.selected_table + 1).min(state.tables.len() - 1);
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.pending_g = false;
            state.selected_table = state.selected_table.saturating_sub(1);
        }
        // G = last table
        KeyCode::Char('G') => {
            state.pending_g = false;
            if !state.tables.is_empty() {
                state.selected_table = state.tables.len() - 1;
            }
        }
        // gg = first table
        KeyCode::Char('g') => {
            if state.pending_g {
                state.selected_table = 0;
                state.pending_g = false;
            } else {
                state.pending_g = true;
            }
        }
        KeyCode::Enter => {
            state.pending_g = false;
            open_selected_table(state, cmd_tx);
        }
        KeyCode::Esc => {
            state.pending_g = false;
            state.focused = FocusedPanel::Editor;
        }
        _ => { state.pending_g = false; }
    }
}

// ---- Editor panel keys ----
fn handle_key_editor(
    state: &mut AppState,
    key: KeyEvent,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    match state.editor_mode {
        // ------------------------------------------------------------------
        // NORMAL mode — hjkl move the cursor; `i` enters Insert
        // ------------------------------------------------------------------
        EditorMode::Normal => {
            // Alt+hjkl are handled globally before this function is called.
            match (key.code, key.modifiers) {
                // Enter Insert mode
                (KeyCode::Char('i'), KeyModifiers::NONE) => {
                    state.editor_mode = EditorMode::Insert;
                }
                // Cursor movement — delegate to tui_textarea CursorMove
                (KeyCode::Char('h') | KeyCode::Left, KeyModifiers::NONE) => {
                    state.editor.move_cursor(CursorMove::Back);
                }
                (KeyCode::Char('l') | KeyCode::Right, KeyModifiers::NONE) => {
                    state.editor.move_cursor(CursorMove::Forward);
                }
                (KeyCode::Char('j') | KeyCode::Down, KeyModifiers::NONE) => {
                    state.editor.move_cursor(CursorMove::Down);
                }
                (KeyCode::Char('k') | KeyCode::Up, KeyModifiers::NONE) => {
                    state.editor.move_cursor(CursorMove::Up);
                }
                // Word jumps (vim: w / b)
                (KeyCode::Char('w'), KeyModifiers::NONE) => {
                    state.editor.move_cursor(CursorMove::WordForward);
                }
                (KeyCode::Char('b'), KeyModifiers::NONE) => {
                    state.editor.move_cursor(CursorMove::WordBack);
                }
                // Line start / end (vim: 0 / $)
                (KeyCode::Char('0'), KeyModifiers::NONE) => {
                    state.editor.move_cursor(CursorMove::Head);
                }
                (KeyCode::Char('$'), KeyModifiers::NONE) => {
                    state.editor.move_cursor(CursorMove::End);
                }
                // File start / end (vim: gg / G) — simple single-key versions
                (KeyCode::Char('g'), KeyModifiers::NONE) => {
                    state.editor.move_cursor(CursorMove::Top);
                }
                (KeyCode::Char('G'), _) => {
                    state.editor.move_cursor(CursorMove::Bottom);
                }
                // Run query still works from Normal mode
                _ if is_run_query(&key) => {
                    let sql = state.editor_sql();
                    if !sql.trim().is_empty() {
                        state.sort_state.clear();
                        state.active_filter = None;
                        let _ = cmd_tx.send(CoreCommand::ExecuteQuery { sql });
                        state.focused = FocusedPanel::Results;
                    }
                }
                // Esc in Normal — no-op (already Normal; use Alt+hjkl to change panel)
                (KeyCode::Esc, _) => {}
                _ => {}
            }
        }

        // ------------------------------------------------------------------
        // INSERT mode — full tui-textarea editing; Esc returns to Normal
        // ------------------------------------------------------------------
        EditorMode::Insert => {
            if is_run_query(&key) {
                let sql = state.editor_sql();
                if !sql.trim().is_empty() {
                    state.sort_state.clear();
                    state.active_filter = None;
                    let _ = cmd_tx.send(CoreCommand::ExecuteQuery { sql });
                    state.focused = FocusedPanel::Results;
                }
                return;
            }
            if key.code == KeyCode::Esc {
                // Return to Normal — do NOT jump to Results panel
                state.editor_mode = EditorMode::Normal;
                return;
            }
            // Forward everything else to the textarea
            state.editor.input(Input::from(key));
        }
    }
}

// ---- Results panel keys ----
fn handle_key_results(
    state: &mut AppState,
    key: KeyEvent,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    tracing::info!(
        "handle_key_results: code={:?} mods={:?} rows={} cols={}",
        key.code, key.modifiers,
        state.results.rows.len(),
        state.results.columns.len(),
    );
    match (key.code, key.modifiers) {
        // Row navigation
        (KeyCode::Down | KeyCode::Char('j'), KeyModifiers::NONE) => {
            state.pending_g = false;
            state.pending_d = false;
            if state.move_row_down_with_page_hint() {
                let next = state.current_page + 1;
                let _ = cmd_tx.send(CoreCommand::FetchPage { page: next });
            }
        }
        (KeyCode::Up | KeyCode::Char('k'), KeyModifiers::NONE) => {
            state.pending_g = false;
            state.pending_d = false;
            state.move_row_up();
        }

        // Column navigation
        (KeyCode::Right | KeyCode::Char('l'), KeyModifiers::NONE) => {
            state.pending_g = false;
            state.pending_d = false;
            state.move_col_right();
        }
        (KeyCode::Left | KeyCode::Char('h'), KeyModifiers::NONE) => {
            state.pending_g = false;
            state.pending_d = false;
            state.move_col_left();
        }

        // vim: gg = first row
        (KeyCode::Char('g'), KeyModifiers::NONE) => {
            state.pending_d = false;
            if state.pending_g {
                state.move_row_first();
                state.pending_g = false;
            } else {
                state.pending_g = true;
            }
        }
        // vim: G = last row
        (KeyCode::Char('G'), KeyModifiers::NONE) | (KeyCode::Char('G'), KeyModifiers::SHIFT) => {
            state.pending_g = false;
            state.pending_d = false;
            state.move_row_last();
        }

        // vim: Ctrl+d = half page down
        (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
            state.pending_g = false;
            state.pending_d = false;
            if state.move_row_half_page_down() {
                let next = state.current_page + 1;
                let _ = cmd_tx.send(CoreCommand::FetchPage { page: next });
            }
        }
        // vim: Ctrl+u = half page up
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            state.pending_g = false;
            state.pending_d = false;
            state.move_row_half_page_up();
        }

        // dd = mark/unmark row for deletion
        (KeyCode::Char('d'), KeyModifiers::NONE) => {
            state.pending_g = false;
            if state.pending_d {
                // Second d — initiate PK lookup to resolve the delete
                let row_idx = state.selected_row;
                let sql = state.editor_sql();
                let (schema, table) = extract_schema_table_from_sql(&sql)
                    .unwrap_or_else(|| ("public".into(), "unknown".into()));
                state.pending_delete_row = Some(row_idx);
                let _ = cmd_tx.send(CoreCommand::GetPrimaryKeys { schema, table });
                state.pending_d = false;
            } else {
                state.pending_d = true;
            }
        }

        // vim: 0 / ^ = first column
        (KeyCode::Char('0'), KeyModifiers::NONE) | (KeyCode::Char('^'), KeyModifiers::NONE) => {
            state.pending_g = false;
            state.pending_d = false;
            state.move_col_first();
        }
        // vim: $ = last column
        (KeyCode::Char('$'), KeyModifiers::NONE) => {
            state.pending_g = false;
            state.pending_d = false;
            state.move_col_last();
        }

        // Page navigation
        (KeyCode::PageDown, _) => {
            state.pending_g = false;
            state.pending_d = false;
            if state.results.has_next_page {
                let next = state.current_page + 1;
                let _ = cmd_tx.send(CoreCommand::FetchPage { page: next });
            }
        }
        (KeyCode::PageUp, _) => {
            state.pending_g = false;
            state.pending_d = false;
            if state.current_page > 0 {
                let prev = state.current_page - 1;
                let _ = cmd_tx.send(CoreCommand::FetchPage { page: prev });
            }
        }

        // Enter = begin cell edit
        (KeyCode::Enter, KeyModifiers::NONE) | (KeyCode::Char('i'), KeyModifiers::NONE) => {
            state.pending_g = false;
            state.pending_d = false;
            enter_cell_edit_mode(state, cmd_tx);
        }

        // Ctrl+W = commit all staged changes to the DB
        _ if is_commit(&key) => {
            state.pending_g = false;
            state.pending_d = false;
            commit_pending(state, cmd_tx);
        }

        // 'o' = toggle sort on selected column
        (KeyCode::Char('o'), KeyModifiers::NONE) => {
            state.pending_g = false;
            state.pending_d = false;
            if let Some(col) = state.selected_column_name().map(str::to_owned) {
                let (col, dir) = state.toggle_sort(&col);
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

        // '/' or Ctrl+F = open filter bar
        (KeyCode::Char('/'), KeyModifiers::NONE)
        | (KeyCode::Char('f'), KeyModifiers::CONTROL) => {
            state.pending_g = false;
            state.pending_d = false;
            state.filter_bar.visible = true;
            state.filter_bar.textarea = tui_textarea::TextArea::default();
        }

        // Esc = discard staged changes and go back to editor
        (KeyCode::Esc, _) => {
            state.pending_g = false;
            state.pending_d = false;
            if !state.pending_edits.is_empty() || !state.pending_deletes.is_empty() {
                state.discard_pending();
                state.status_msg = Some("Staged changes discarded.".into());
            } else {
                state.focused = FocusedPanel::Editor;
            }
        }

        _ => {
            state.pending_g = false;
            state.pending_d = false;
        }
    }
}

// ---- Cell edit mode keys ----
fn handle_key_cell_edit(
    state: &mut AppState,
    key: KeyEvent,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => {
            // Cancel — discard the overlay without staging anything
            state.cell_edit = None;
        }
        (KeyCode::Char('s'), KeyModifiers::CONTROL) | (KeyCode::Enter, KeyModifiers::NONE) => {
            stage_cell_edit(state);
        }
        _ if is_commit(&key) => {
            // Allow Ctrl+W directly from the overlay: stage current value first,
            // then commit all staged edits/deletes.
            stage_cell_edit(state);
            commit_pending(state, cmd_tx);
        }
        _ => {
            if let Some(ce) = state.cell_edit.as_mut() {
                ce.textarea.input(Input::from(key));
            }
        }
    }
}

fn stage_cell_edit(state: &mut AppState) {
    // Stage the edit locally (no DB write yet)
    if let Some(ce) = state.cell_edit.take() {
        let new_val = ce.current_value();
        let col_name = ce.col_name.clone();
        if new_val != ce.original {
            state.pending_edits.insert(
                (ce.row_idx, ce.col_idx),
                app::PendingEdit {
                    new_val,
                    original: ce.original,
                    schema: ce.schema,
                    table: ce.table,
                    pk_col: ce.pk_col,
                    pk_val: ce.pk_val,
                    col_name: ce.col_name,
                },
            );
            let total = state.pending_edits.len() + state.pending_deletes.len();
            state.status_msg = Some(format!(
                "Staged edit on '{}'. Total staged: {}. Press Ctrl+W to commit.",
                col_name, total
            ));
        } else {
            state.status_msg = Some("No changes to stage (value unchanged).".into());
        }
    }
}

// ---- Filter bar keys ----
fn handle_key_filter(
    state: &mut AppState,
    key: KeyEvent,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    match key.code {
        KeyCode::Esc => {
            state.filter_bar.visible = false;
            state.active_filter = None;
            let _ = cmd_tx.send(CoreCommand::ClearFilter);
        }
        KeyCode::Enter => {
            let query = state.filter_bar.textarea.lines().join("");
            state.filter_bar.visible = false;
            if query.trim().is_empty() {
                state.active_filter = None;
                let _ = cmd_tx.send(CoreCommand::ClearFilter);
            } else {
                state.active_filter = Some(query.clone());
                let _ = cmd_tx.send(CoreCommand::ApplyFilter { query });
            }
        }
        _ => {
            state.filter_bar.textarea.input(Input::from(key));
        }
    }
}

// ---- Connection form keys ----
fn handle_key_conn_form(
    state: &mut AppState,
    key: KeyEvent,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    match key.code {
        KeyCode::Esc => {
            state.conn_form.visible = false;
        }
        KeyCode::Tab | KeyCode::Down => {
            state.conn_form.field_index =
                (state.conn_form.field_index + 1) % app::ConnectionForm::field_count();
        }
        KeyCode::BackTab | KeyCode::Up => {
            state.conn_form.field_index = state
                .conn_form
                .field_index
                .checked_sub(1)
                .unwrap_or(app::ConnectionForm::field_count() - 1);
        }
        KeyCode::Enter => {
            submit_conn_form(state, cmd_tx);
        }
        // Space on SSL Mode field cycles through modes
        KeyCode::Char(' ') if state.conn_form.field_index == 6 => {
            state.conn_form.cycle_ssl_mode();
        }
        KeyCode::Backspace => {
            if let Some(val) = state.conn_form.active_value_mut() {
                val.pop();
            }
        }
        KeyCode::Char(c) => {
            if let Some(val) = state.conn_form.active_value_mut() {
                val.push(c);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Mouse handler
// ---------------------------------------------------------------------------

fn handle_mouse(
    state: &mut AppState,
    mouse: MouseEvent,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    if let Some(ref mut diag) = state.diagram {
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                if mouse.modifiers.contains(KeyModifiers::SHIFT)
                    || mouse.modifiers.contains(KeyModifiers::ALT)
                {
                    diag.scroll_x = diag.scroll_x.saturating_add(4);
                } else {
                    diag.scroll_y = diag.scroll_y.saturating_add(2);
                }
            }
            MouseEventKind::ScrollUp => {
                if mouse.modifiers.contains(KeyModifiers::SHIFT)
                    || mouse.modifiers.contains(KeyModifiers::ALT)
                {
                    diag.scroll_x = diag.scroll_x.saturating_sub(4);
                } else {
                    diag.scroll_y = diag.scroll_y.saturating_sub(2);
                }
            }
            _ => {}
        }
        return;
    }

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some(la) = state.last_areas {
                let col = mouse.column;
                let row = mouse.row;
                if rect_contains(la.table_list, col, row) {
                    // Click in the table list — focus Tables panel and select item
                    state.focused = FocusedPanel::Tables;
                    state.nav_mode = NavMode::Panel;
                    // row 0 = top border, row 1+ = table items
                    if row > la.table_list.y {
                        let clicked = (row - la.table_list.y).saturating_sub(1) as usize;
                        let new_idx = clicked.min(state.tables.len().saturating_sub(1));
                        state.selected_table = new_idx;
                    }
                } else if rect_contains(la.conn_list, col, row) {
                    // Click in the connection list — focus Connections panel and select item
                    state.focused = FocusedPanel::Connections;
                    state.nav_mode = NavMode::Panel;
                    if row > la.conn_list.y {
                        let clicked = (row - la.conn_list.y).saturating_sub(1) as usize;
                        if !state.connections.is_empty() {
                            state.selected_connection =
                                clicked.min(state.connections.len() - 1);
                        }
                    }
                } else if rect_contains(la.editor, col, row) {
                    state.focused = FocusedPanel::Editor;
                    state.nav_mode = NavMode::Panel;
                } else if rect_contains(la.results, col, row) {
                    state.focused = FocusedPanel::Results;
                    state.nav_mode = NavMode::Panel;
                    // Compute which data row was clicked:
                    // row 0 = top border, row 1 = header, row 2+ = data rows
                    let header_offset = 2u16; // border + header
                    if row >= la.results.y + header_offset {
                        let clicked_row_vis = (row - la.results.y - header_offset) as usize;
                        let new_row = state.result_scroll + clicked_row_vis;
                        if new_row < state.results.rows.len() {
                            state.selected_row = new_row;
                        }
                    }
                    // Compute which column was clicked using stored widths
                    if !state.last_col_widths.is_empty() && col > la.results.x {
                        let inner_x = (col - la.results.x).saturating_sub(1) as usize;
                        let col_scroll = state.result_col_scroll;
                        let mut acc = 0usize;
                        let mut clicked_col = col_scroll;
                        for (ci, &w) in state.last_col_widths.iter().enumerate().skip(col_scroll) {
                            let next_acc = acc + w as usize + 1; // +1 for COL_SPACING
                            if inner_x < next_acc {
                                clicked_col = ci;
                                break;
                            }
                            acc = next_acc;
                            clicked_col = ci + 1;
                        }
                        let max_col = state.results.columns.len().saturating_sub(1);
                        state.selected_col = clicked_col.min(max_col);
                    }
                }
            } else {
                // Fallback heuristic before first draw
                let term_width = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);
                let conn_width = term_width / 4;
                if mouse.column < conn_width {
                    state.focused = FocusedPanel::Connections;
                    state.nav_mode = NavMode::Panel;
                } else {
                    let term_height = crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24);
                    let editor_height = term_height * 35 / 100;
                    if mouse.row < editor_height {
                        state.focused = FocusedPanel::Editor;
                        state.nav_mode = NavMode::Panel;
                    } else {
                        state.focused = FocusedPanel::Results;
                        state.nav_mode = NavMode::Panel;
                    }
                }
            }
        }
        MouseEventKind::ScrollDown => {
            match state.focused {
                FocusedPanel::Results => {
                    if state.move_row_down_with_page_hint() {
                        let next = state.current_page + 1;
                        let _ = cmd_tx.send(CoreCommand::FetchPage { page: next });
                    }
                }
                FocusedPanel::Connections => {
                    if !state.connections.is_empty() {
                        let next = state.selected_connection + 1;
                        if next < state.connections.len() {
                            state.selected_connection = next;
                        }
                    }
                }
                FocusedPanel::Tables => {
                    if !state.tables.is_empty() {
                        state.selected_table =
                            (state.selected_table + 1).min(state.tables.len() - 1);
                    }
                }
                _ => {}
            }
        }
        MouseEventKind::ScrollUp => {
            match state.focused {
                FocusedPanel::Results => {
                    state.move_row_up();
                }
                FocusedPanel::Connections => {
                    state.selected_connection =
                        state.selected_connection.saturating_sub(1);
                }
                FocusedPanel::Tables => {
                    state.selected_table = state.selected_table.saturating_sub(1);
                }
                _ => {}
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Diagram mode key handler
// ---------------------------------------------------------------------------

fn handle_key_diagram(state: &mut AppState, key: KeyEvent) {
    let Some(ref mut diag) = state.diagram else {
        return;
    };
    let visible = diagram_visible_table_indices(diag);
    if !visible.is_empty() && !visible.contains(&diag.selected_table) {
        diag.selected_table = visible[0];
    }
    let visible_pos = visible
        .iter()
        .position(|&idx| idx == diag.selected_table)
        .unwrap_or(0);
    let table_count = visible.len();

    match key.code {
        // Exit diagram
        KeyCode::Esc | KeyCode::Char('q') => {
            state.diagram = None;
        }

        // Scroll canvas: hjkl / arrows
        KeyCode::Left | KeyCode::Char('h') => {
            diag.scroll_x = diag.scroll_x.saturating_sub(4);
        }
        KeyCode::Right | KeyCode::Char('l') => {
            diag.scroll_x = diag.scroll_x.saturating_add(4);
        }
        KeyCode::Up => {
            diag.scroll_y = diag.scroll_y.saturating_sub(1);
        }
        KeyCode::Down => {
            diag.scroll_y = diag.scroll_y.saturating_add(1);
        }

        // Navigate table list: j/k / Tab
        KeyCode::Char('j') | KeyCode::Tab => {
            if table_count > 0 {
                let next_pos = (visible_pos + 1).min(table_count - 1);
                diag.selected_table = visible[next_pos];
            }
        }
        KeyCode::Char('k') | KeyCode::BackTab => {
            if table_count > 0 {
                let prev_pos = visible_pos.saturating_sub(1);
                diag.selected_table = visible[prev_pos];
            }
        }
        KeyCode::Char('g') => {
            if table_count > 0 {
                diag.selected_table = visible[0];
            }
            diag.scroll_y = 0;
        }
        KeyCode::Char('G') => {
            if table_count > 0 {
                diag.selected_table = visible[table_count - 1];
            }
        }
        KeyCode::Char('f') => {
            diag.focus_mode = !diag.focus_mode;
            if diag.focus_mode {
                let vis = diagram_visible_table_indices(diag);
                if !vis.is_empty() && !vis.contains(&diag.selected_table) {
                    diag.selected_table = vis[0];
                }
            }
        }
        KeyCode::Char('u') => {
            diag.glyph_mode = match diag.glyph_mode {
                app::DiagramGlyphMode::Ascii => app::DiagramGlyphMode::Unicode,
                app::DiagramGlyphMode::Unicode => app::DiagramGlyphMode::Ascii,
            };
        }
        // Enter / space = jump canvas to selected table
        KeyCode::Enter | KeyCode::Char(' ') => {
            jump_canvas_to_table(diag);
        }
        _ => {}
    }
}

/// Scroll the canvas viewport so the selected table is visible.
fn jump_canvas_to_table(diag: &mut app::DiagramState) {
    let visible = diagram_visible_table_indices(diag);
    let idx = visible
        .iter()
        .position(|&i| i == diag.selected_table)
        .unwrap_or(0);
    let row = idx / ui::diagram::COLS_PER_ROW_PUB;
    let new_y = (row as u16) * (ui::diagram::BOX_ROW_HEIGHT + ui::diagram::V_GAP_PUB);
    diag.scroll_y = new_y;
}

fn diagram_visible_table_indices(diag: &app::DiagramState) -> Vec<usize> {
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

// ---------------------------------------------------------------------------
// Table open helper
// ---------------------------------------------------------------------------

fn open_selected_table(
    state: &mut AppState,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    if let Some(t) = state.tables.get(state.selected_table) {
        let sql = sbql_core::query_builder::table_select_sql(&t.schema, &t.name);
        tracing::info!("open_selected_table: schema={:?} table={:?} sql={:?}", t.schema, t.name, sql);
        state.sort_state.clear();
        state.active_filter = None;
        state.editor = {
            let mut ta = tui_textarea::TextArea::default();
            ta.set_placeholder_text("-- Write SQL here. Press Ctrl+S or F5 to run.");
            ta.insert_str(&sql);
            ta
        };
        let _ = cmd_tx.send(CoreCommand::ExecuteQuery { sql });
        state.focused = FocusedPanel::Results;
    }
}

// ---------------------------------------------------------------------------
// Commit staged changes
// ---------------------------------------------------------------------------

/// Send all pending edits and deletes to Core, then refresh the current page.
fn commit_pending(
    state: &mut AppState,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    if state.pending_edits.is_empty() && state.pending_deletes.is_empty() {
        state.error_msg = Some("Nothing to commit — no staged edits or deletes.".into());
        return;
    }

    let edit_count = state.pending_edits.len();
    let delete_count = state.pending_deletes.len();

    // Send all UPDATE commands
    for edit in state.pending_edits.values() {
        let _ = cmd_tx.send(CoreCommand::UpdateCell {
            schema: edit.schema.clone(),
            table: edit.table.clone(),
            pk_col: edit.pk_col.clone(),
            pk_val: edit.pk_val.clone(),
            target_col: edit.col_name.clone(),
            new_val: edit.new_val.clone(),
        });
    }

    // Send all DELETE commands
    for del in state.pending_deletes.values() {
        let _ = cmd_tx.send(CoreCommand::DeleteRow {
            schema: del.schema.clone(),
            table: del.table.clone(),
            pk_col: del.pk_col.clone(),
            pk_val: del.pk_val.clone(),
        });
    }

    // Clear staged state
    state.pending_edits.clear();
    state.pending_deletes.clear();
    state.pending_d = false;

    // Refresh the page after all commands are sent
    let page = state.current_page;
    let _ = cmd_tx.send(CoreCommand::FetchPage { page });

    state.status_msg = Some(format!(
        "Committed: {} edit(s), {} delete(s).",
        edit_count, delete_count
    ));
}

// ---------------------------------------------------------------------------
// Cell edit entry point
// ---------------------------------------------------------------------------

fn enter_cell_edit_mode(
    state: &mut AppState,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    let row_idx = state.selected_row;
    let col_idx = state.selected_col;

    // Make sure there is actually a cell at the cursor
    if state.results.columns.get(col_idx).is_none() {
        return;
    }
    if state.results.rows.get(row_idx).is_none() {
        return;
    }

    // Extract schema and table name from the current SQL query
    let sql = state.editor_sql();
    let parsed = extract_schema_table_from_sql(&sql);
    tracing::info!("enter_cell_edit_mode: sql={:?} parsed={:?}", sql, parsed);
    let (schema, table_name) = parsed
        .unwrap_or_else(|| ("public".into(), "unknown".into()));

    // Store the pending context and ask Core for the PK definition
    state.pending_cell_edit = Some((row_idx, col_idx));
    tracing::info!("GetPrimaryKeys: schema={:?} table={:?}", schema, table_name);
    let _ = cmd_tx.send(CoreCommand::GetPrimaryKeys {
        schema,
        table: table_name,
    });
}

/// Extract `(schema, table)` from the first `FROM <name>` in the SQL.
/// Handles:
///   - `FROM table`
///   - `FROM schema.table`
///   - `FROM "quoted"."table"`
///   - `FROM table alias` / `FROM table AS alias`
///   - Defaults schema to `"public"` if not qualified.
fn extract_schema_table_from_sql(sql: &str) -> Option<(String, String)> {
    let upper = sql.to_uppercase();
    let from_pos = upper.find("FROM ")?;
    let rest = sql[from_pos + 5..].trim_start();

    // Parse a single identifier (unquoted or double-quoted)
    fn parse_ident(s: &str) -> (&str, &str) {
        if s.starts_with('"') {
            // quoted identifier — find closing quote
            let inner = &s[1..];
            if let Some(end) = inner.find('"') {
                (&inner[..end], &inner[end + 1..])
            } else {
                (inner, "")
            }
        } else {
            // unquoted — take alphanumeric / underscore / hyphen chars
            let end = s
                .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
                .unwrap_or(s.len());
            (&s[..end], &s[end..])
        }
    }

    let (first, after_first) = parse_ident(rest);
    if first.is_empty() {
        return None;
    }

    // Check if next non-whitespace char is '.' — schema-qualified name
    let after_first = after_first.trim_start();
    if after_first.starts_with('.') {
        let after_dot = after_first[1..].trim_start();
        let (second, _) = parse_ident(after_dot);
        if second.is_empty() {
            return None;
        }
        Some((first.to_owned(), second.to_owned()))
    } else {
        Some(("public".to_owned(), first.to_owned()))
    }
}

// ---------------------------------------------------------------------------
// Connection form submission
// ---------------------------------------------------------------------------

fn submit_conn_form(
    state: &mut AppState,
    cmd_tx: &mpsc::UnboundedSender<CoreCommand>,
) {
    let form = &state.conn_form;

    if form.name.trim().is_empty() {
        state.conn_form.error = Some("Name is required".into());
        return;
    }
    if form.host.trim().is_empty() {
        state.conn_form.error = Some("Host is required".into());
        return;
    }
    let port: u16 = match form.port.trim().parse() {
        Ok(p) => p,
        Err(_) => {
            state.conn_form.error = Some("Port must be a number (1-65535)".into());
            return;
        }
    };
    if form.user.trim().is_empty() {
        state.conn_form.error = Some("User is required".into());
        return;
    }
    if form.database.trim().is_empty() {
        state.conn_form.error = Some("Database is required".into());
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

    // When editing an existing connection, preserve the original UUID so the
    // SaveConnection handler correctly upserts rather than creating a duplicate.
    if let Some(id) = form.editing_id {
        config.id = id;
    }

    let password = if form.password.is_empty() && form.editing_id.is_some() {
        // Editing an existing connection without re-entering the password —
        // tell Core to keep the stored password unchanged.
        None
    } else {
        Some(form.password.clone())
    };
    let _ = cmd_tx.send(CoreCommand::SaveConnection { config, password });
    state.conn_form.visible = false;
    state.conn_form.error = None;
}

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

/// Returns true if the terminal cell (col, row) falls within `rect`.
fn rect_contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x
        && col < rect.x + rect.width
        && row >= rect.y
        && row < rect.y + rect.height
}
