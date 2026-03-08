use std::io;

use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, supports_keyboard_enhancement, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use ratatui::{backend::CrosstermBackend, Terminal};
use sbql_core::load_connections;
use sbql_core::CoreCommand;
use tokio::sync::mpsc;

mod action;
mod app;
mod completion;
mod events;
mod handlers;
mod highlight;
#[cfg(test)]
mod test_helpers;
mod ui;
mod worker;

use app::AppState;
use events::{spawn_event_reader, AppEvent};
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
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
            loop {
                interval.tick().await;
                if app_tx3.send(AppEvent::Tick).is_err() {
                    break;
                }
            }
        });
    }

    let mut state = AppState::new(Vec::new());
    let mut auto_connected = false;
    terminal.draw(|f| ui::draw(f, &mut state))?;

    loop {
        let event = match app_rx.recv().await {
            Some(e) => e,
            None => break,
        };

        match event {
            AppEvent::Core(ce) => {
                tracing::debug!("CoreEvent: {:?}", ce);
                let auto_list = matches!(ce, sbql_core::CoreEvent::Connected(_));
                let tables_loaded = matches!(ce, sbql_core::CoreEvent::TableList(_));
                if !auto_connected {
                    if let sbql_core::CoreEvent::ConnectionList(ref conns) = ce {
                        if let Some(ref name) = auto_connect_name {
                            if let Some(cfg) =
                                conns.iter().find(|c| c.name.eq_ignore_ascii_case(name))
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
                if tables_loaded {
                    // Pre-load diagram data so autocomplete has column info.
                    let _ = cmd_tx.send(CoreCommand::LoadDiagram);
                }
            }

            AppEvent::Resize(_, _) => {
                state.layout.needs_redraw = true;
            }

            AppEvent::IoError(e) => {
                state.error_msg = Some(format!("IO error: {e}"));
                state.layout.needs_redraw = true;
            }

            AppEvent::Key(key) => {
                let act = handlers::handle_key(&state, key);
                action::apply(act, &mut state, &cmd_tx);
                state.layout.needs_redraw = true;
                if state.should_quit {
                    break;
                }
            }

            AppEvent::Mouse(mouse) => {
                handlers::mouse::handle(&mut state, mouse, &cmd_tx);
                state.layout.needs_redraw = true;
            }

            AppEvent::Tick => {
                if state.results.is_loading {
                    state.layout.spinner_frame = state.layout.spinner_frame.wrapping_add(1);
                    state.layout.needs_redraw = true;
                }
                if action::apply_live_filter_if_due(&mut state, &cmd_tx) {
                    state.layout.needs_redraw = true;
                }
            }
        }

        if state.layout.needs_redraw {
            terminal.draw(|f| ui::draw(f, &mut state))?;
            state.layout.needs_redraw = false;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    if keyboard_enhanced {
        execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags)?;
    }
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyCode;

    use crate::app::{EditorMode, FocusedPanel, NavMode};
    use crate::test_helpers::*;

    #[test]
    fn test_global_navigation_and_modes() {
        let mut state = make_state_with_results();
        let (cmd_tx, _cmd_rx) = cmd_channel();

        state.focused = FocusedPanel::Connections;

        dispatch(&mut state, key(KeyCode::Tab), &cmd_tx);
        assert_eq!(state.focused, FocusedPanel::Tables);

        dispatch(&mut state, key(KeyCode::F(3)), &cmd_tx);
        assert_eq!(state.focused, FocusedPanel::Editor);

        dispatch(&mut state, key(KeyCode::Enter), &cmd_tx);
        assert_eq!(state.vim.nav_mode, NavMode::Panel);
        assert_eq!(state.editor.mode, EditorMode::Normal);

        dispatch(&mut state, key(KeyCode::Char('i')), &cmd_tx);
        assert_eq!(state.editor.mode, EditorMode::Insert);

        dispatch(&mut state, key(KeyCode::Esc), &cmd_tx);
        assert_eq!(state.editor.mode, EditorMode::Normal);
        assert_eq!(state.vim.nav_mode, NavMode::Panel);

        dispatch(&mut state, key(KeyCode::Esc), &cmd_tx);
        assert_eq!(state.vim.nav_mode, NavMode::Global);
    }

    #[tokio::test]
    async fn test_editor_input_and_query_execution() {
        let mut state = make_state_with_results();
        let (cmd_tx, mut cmd_rx) = cmd_channel();

        state.focused = FocusedPanel::Editor;
        state.vim.nav_mode = NavMode::Panel;
        state.editor.mode = EditorMode::Insert;

        dispatch(&mut state, key(KeyCode::Char('S')), &cmd_tx);
        dispatch(&mut state, key(KeyCode::Char('E')), &cmd_tx);
        dispatch(&mut state, key(KeyCode::Char('L')), &cmd_tx);

        assert_eq!(state.editor.sql(), "SEL");

        dispatch(&mut state, key(KeyCode::F(5)), &cmd_tx);
        assert_eq!(state.focused, FocusedPanel::Results);

        let cmd = cmd_rx.recv().await.expect("Expected a command");
        match cmd {
            sbql_core::CoreCommand::ExecuteQuery { sql } => assert_eq!(sql, "SEL"),
            _ => panic!("Expected ExecuteQuery command"),
        }
    }

    #[tokio::test]
    async fn test_results_table_navigation() {
        let mut state = make_state_with_results();
        let (cmd_tx, _cmd_rx) = cmd_channel();

        state.focused = FocusedPanel::Results;
        state.vim.nav_mode = NavMode::Panel;
        state.results.selected_row = 0;
        state.results.selected_col = 0;

        dispatch(&mut state, key(KeyCode::Char('j')), &cmd_tx);
        assert_eq!(state.results.selected_row, 1);

        dispatch(&mut state, key(KeyCode::Char('l')), &cmd_tx);
        assert_eq!(state.results.selected_col, 1);

        dispatch(
            &mut state,
            key_mod(KeyCode::Char('G'), crossterm::event::KeyModifiers::SHIFT),
            &cmd_tx,
        );
        assert_eq!(state.results.selected_row, 4);

        dispatch(&mut state, key(KeyCode::Char('$')), &cmd_tx);
        assert_eq!(state.results.selected_col, 2);
    }
}
