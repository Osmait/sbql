//! The application itself.
//!
//! `Sbql` owns everything that lives for the length of a run — the state, the
//! render cache, the channels to the core worker — and drives the event loop.
//! `main` builds one and calls [`Sbql::run`]; it holds no application logic of
//! its own.
//!
//! The loop is the controller in the usual TUI split: an event arrives, a pure
//! handler turns it into an [`Action`], the reducer applies it, and the view
//! draws the result. Nothing here decides *what* a key means or *how* anything
//! looks.

use anyhow::Result;
use ratatui::backend::Backend;
use sbql_core::{CoreCommand, CoreEvent};
use tokio::sync::mpsc;

use crate::action;
use crate::app::AppState;
use crate::events::{spawn_event_reader, AppEvent};
use crate::handlers;
use crate::session;
use crate::tui::Tui;
use crate::worker::spawn_worker;

/// How often the spinner advances.
const TICK: std::time::Duration = std::time::Duration::from_millis(100);

pub struct Sbql {
    state: AppState,
    /// Commands out to the core worker.
    cmd_tx: mpsc::UnboundedSender<CoreCommand>,
    /// Every kind of event, merged into one stream.
    events: mpsc::UnboundedReceiver<AppEvent>,
    /// Connection named on the command line, if any.
    startup_connection: Option<String>,
    /// Whether the startup connection attempt has already been made.
    auto_connected: bool,
}

impl Sbql {
    /// Wire up the worker, the input reader and the ticker.
    pub fn new(startup_connection: Option<String>) -> Self {
        let (cmd_tx, mut core_rx) = spawn_worker();
        let (app_tx, events) = mpsc::unbounded_channel::<AppEvent>();

        spawn_event_reader(app_tx.clone());

        // Core replies and clock ticks join the same stream as input, so the
        // loop has a single thing to await.
        let core_tx = app_tx.clone();
        tokio::spawn(async move {
            while let Some(ev) = core_rx.recv().await {
                if core_tx.send(AppEvent::Core(ev)).is_err() {
                    break;
                }
            }
        });

        let tick_tx = app_tx;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(TICK);
            loop {
                interval.tick().await;
                if tick_tx.send(AppEvent::Tick).is_err() {
                    break;
                }
            }
        });

        Self::with_channels(cmd_tx, events, startup_connection)
    }

    /// Build around channels somebody else owns.
    ///
    /// Production goes through [`Sbql::new`]; tests use this to feed the loop
    /// synthetic events instead of real key presses.
    pub fn with_channels(
        cmd_tx: mpsc::UnboundedSender<CoreCommand>,
        events: mpsc::UnboundedReceiver<AppEvent>,
        startup_connection: Option<String>,
    ) -> Self {
        Self {
            state: AppState::new(Vec::new()),
            cmd_tx,
            events,
            startup_connection,
            auto_connected: false,
        }
    }

    /// Run until the user quits or the event stream closes.
    ///
    /// Generic over the backend so the whole loop can be driven in a test.
    pub async fn run<B: Backend>(&mut self, tui: &mut Tui<B>) -> Result<()> {
        self.draw(tui)?;

        while let Some(event) = self.events.recv().await {
            match event {
                AppEvent::Core(ev) => self.on_core_event(ev),
                AppEvent::Key(key) => {
                    let action = handlers::handle_key(&self.state, key);
                    self.dispatch(action);
                }
                AppEvent::Mouse(mouse) => {
                    let action = handlers::mouse::handle(&self.state, mouse);
                    self.dispatch(action);
                }
                AppEvent::Resize => self.state.layout.needs_redraw = true,
                AppEvent::IoError(e) => {
                    self.state.error_msg = Some(format!("IO error: {e}"));
                    self.state.layout.needs_redraw = true;
                }
                AppEvent::Tick => self.on_tick(),
            }

            if self.state.should_quit {
                break;
            }
            if self.state.layout.needs_redraw {
                self.draw(tui)?;
                self.state.layout.needs_redraw = false;
            }
        }

        Ok(())
    }

    fn dispatch(&mut self, action: action::Action) {
        action::apply(action, &mut self.state, &self.cmd_tx);
        self.state.layout.needs_redraw = true;
    }

    fn draw<B: Backend>(&mut self, tui: &mut Tui<B>) -> Result<()> {
        tui.draw(&mut self.state)?;
        Ok(())
    }

    fn on_tick(&mut self) {
        if self.state.results.is_loading {
            self.state.layout.spinner_frame = self.state.layout.spinner_frame.wrapping_add(1);
            self.state.layout.needs_redraw = true;
        }
        if action::apply_live_filter_if_due(&mut self.state, &self.cmd_tx) {
            self.state.layout.needs_redraw = true;
        }
    }

    fn on_core_event(&mut self, event: CoreEvent) {
        tracing::debug!("CoreEvent: {:?}", event);

        // Follow-up work the event implies, decided before the state consumes it.
        let connected = matches!(event, CoreEvent::Connected(_));
        let tables_loaded = matches!(event, CoreEvent::TableList(_));

        match &event {
            CoreEvent::Connected(id) => session::remember(id),
            CoreEvent::Disconnected(_) => session::forget(),
            CoreEvent::ConnectionList(conns) if !self.auto_connected => {
                self.try_auto_connect(conns);
            }
            _ => {}
        }

        self.state.apply_core_event(event);

        if connected {
            let _ = self.cmd_tx.send(CoreCommand::ListTables);
        }
        if tables_loaded {
            // Column info for autocomplete rides along with the diagram data.
            let _ = self.cmd_tx.send(CoreCommand::LoadDiagram);
        }

        self.state.layout.needs_redraw = true;
    }

    /// Open a connection on startup: the one named on the command line if it
    /// was given, otherwise whichever was open last.
    fn try_auto_connect(&mut self, conns: &[sbql_core::ConnectionConfig]) {
        let target = self
            .startup_connection
            .as_ref()
            .and_then(|name| conns.iter().find(|c| c.name.eq_ignore_ascii_case(name)))
            .or_else(|| {
                let last = session::last_connection_id()?;
                conns.iter().find(|c| c.id.to_string() == last)
            });

        if let Some(cfg) = target {
            let _ = self.cmd_tx.send(CoreCommand::Connect(cfg.id));
            self.auto_connected = true;
        }
    }
}

#[cfg(test)]
mod tests {
    //! Cover the pipeline `Sbql` drives: a key becomes an action, the reducer
    //! applies it, and the resulting state is what the view would render.

    use super::*;
    use crossterm::event::KeyCode;
    use ratatui::backend::TestBackend;

    use crate::events::AppEvent;
    use crate::tui::Tui;

    /// Drive the real loop over a scripted event stream.
    ///
    /// Dropping the sender ends the stream, so `run` returns on its own. This
    /// is what the `Backend` generic bought: the loop, the reducer and the view
    /// all execute, with no terminal attached.
    async fn run_with(events: Vec<crate::events::AppEvent>) -> (Tui<TestBackend>, AppState) {
        // `Connected` persists the last-connection file; keep that off the
        // developer's machine.
        let scratch = tempfile::tempdir().expect("temp dir");
        std::env::set_var(sbql_core::CONFIG_DIR_ENV, scratch.path());

        let (cmd_tx, _cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        for e in events {
            event_tx.send(e).expect("queue event");
        }
        drop(event_tx);

        let mut tui = Tui::with_backend(TestBackend::new(120, 30)).expect("test backend");
        let mut app = Sbql::with_channels(cmd_tx, event_rx, None);
        app.run(&mut tui).await.expect("run");
        (tui, app.state)
    }

    #[tokio::test]
    async fn the_loop_paints_the_workspace() {
        let (tui, _) = run_with(vec![]).await;
        let screen = tui.rendered();
        for panel in ["Connections", "Tables", "SQL Editor", "Results"] {
            assert!(screen.contains(panel), "{panel} missing from:\n{screen}");
        }
    }

    /// The full round trip: a key press asks the core for something, the reply
    /// arrives as an event, and the result reaches the screen.
    ///
    /// `D` alone does not open the diagram — it only requests the data. The
    /// overlay appears when `DiagramLoaded` comes back, which is why this test
    /// has to play the core's part.
    #[tokio::test]
    async fn a_key_press_and_the_reply_it_asks_for_reach_the_screen() {
        let data = sbql_core::DiagramData {
            tables: vec![sbql_core::TableSchema {
                schema: "public".into(),
                name: "customers".into(),
                columns: vec![sbql_core::ColumnInfo {
                    name: "id".into(),
                    data_type: "integer".into(),
                    is_pk: true,
                    is_nullable: false,
                }],
            }],
            foreign_keys: vec![],
        };

        // The diagram is only offered once a connection is open.
        let conn = sbql_core::ConnectionConfig::new_sqlite("demo", "/tmp/demo.db");
        let id = conn.id;

        let (tui, state) = run_with(vec![
            AppEvent::Core(CoreEvent::ConnectionList(vec![conn])),
            AppEvent::Core(CoreEvent::Connected(id)),
            AppEvent::Key(key(KeyCode::Char('D'))),
            AppEvent::Core(CoreEvent::DiagramLoaded(data)),
        ])
        .await;

        assert_eq!(
            state.mode(),
            crate::app::Mode::Diagram,
            "the reply should have opened the overlay"
        );
        let screen = tui.rendered();
        assert!(screen.contains("Diagram"), "diagram not painted:\n{screen}");
        assert!(screen.contains("customers"), "table missing:\n{screen}");
    }

    /// Quitting ends the loop rather than relying on the stream closing.
    #[tokio::test]
    async fn quitting_stops_the_loop() {
        let (cmd_tx, _cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        event_tx
            .send(AppEvent::Key(key(KeyCode::Char('q'))))
            .unwrap();
        // Still open, and still holding queued work the loop must not reach.
        event_tx
            .send(AppEvent::Key(key(KeyCode::Char('D'))))
            .unwrap();

        let mut tui = Tui::with_backend(TestBackend::new(80, 24)).expect("test backend");
        let mut app = Sbql::with_channels(cmd_tx, event_rx, None);
        app.run(&mut tui).await.expect("run");

        assert!(app.state.should_quit);
        assert_ne!(
            app.state.mode(),
            crate::app::Mode::Diagram,
            "events after quit must not be processed"
        );
    }

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
