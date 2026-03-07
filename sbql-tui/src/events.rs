//! Crossterm event reader and the unified `AppEvent` enum.
//!
//! A dedicated OS thread blocks on `crossterm::event::read()` and forwards
//! events into a `mpsc::Sender<AppEvent>`.  Core events arrive on the same
//! channel so the main loop has a single point to drain.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use sbql_core::CoreEvent;
use tokio::sync::mpsc;

/// All events the main loop needs to handle.
#[derive(Debug)]
pub enum AppEvent {
    /// A keyboard event.
    Key(KeyEvent),
    /// A mouse event.
    Mouse(MouseEvent),
    /// Terminal was resized.
    #[allow(dead_code)]
    Resize(u16, u16),
    /// A response arrived from the Core worker.
    Core(CoreEvent),
    /// The crossterm reader thread encountered an error.
    IoError(String),
    /// Periodic tick for spinner animation (fired every ~100 ms).
    Tick,
}

/// Spawn a background thread that reads crossterm events and sends them as
/// `AppEvent` values on `tx`.
///
/// The thread runs until the channel is closed (i.e. the receiver is dropped).
pub fn spawn_event_reader(tx: mpsc::UnboundedSender<AppEvent>) {
    std::thread::spawn(move || loop {
        match crossterm::event::read() {
            Ok(Event::Key(k)) => {
                if tx.send(AppEvent::Key(k)).is_err() {
                    break;
                }
            }
            Ok(Event::Mouse(m)) => {
                if tx.send(AppEvent::Mouse(m)).is_err() {
                    break;
                }
            }
            Ok(Event::Resize(w, h)) => {
                if tx.send(AppEvent::Resize(w, h)).is_err() {
                    break;
                }
            }
            Ok(_) => {} // FocusGained/Lost etc — ignore
            Err(e) => {
                let _ = tx.send(AppEvent::IoError(e.to_string()));
                break;
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Key binding helpers (used by main.rs)
// ---------------------------------------------------------------------------

/// Returns true for `q` without modifiers, or `Ctrl+C` / `Ctrl+Q`.
pub fn is_quit(k: &KeyEvent) -> bool {
    matches!(
        (k.code, k.modifiers),
        (KeyCode::Char('q'), KeyModifiers::NONE)
            | (KeyCode::Char('c'), KeyModifiers::CONTROL)
            | (KeyCode::Char('q'), KeyModifiers::CONTROL)
    )
}

/// Returns true for `Ctrl+S` or `F5` — the "run query" binding.
pub fn is_run_query(k: &KeyEvent) -> bool {
    matches!(
        (k.code, k.modifiers),
        (KeyCode::Char('s'), KeyModifiers::CONTROL) | (KeyCode::F(5), KeyModifiers::NONE)
    )
}

/// Returns true for `Ctrl+W` — commit staged changes to the DB.
pub fn is_commit(k: &KeyEvent) -> bool {
    matches!(
        (k.code, k.modifiers),
        (KeyCode::Char('w'), KeyModifiers::CONTROL)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState};

    fn make_key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    // -- is_quit --

    #[test]
    fn q_is_quit() {
        assert!(is_quit(&make_key(KeyCode::Char('q'), KeyModifiers::NONE)));
    }

    #[test]
    fn ctrl_c_is_quit() {
        assert!(is_quit(&make_key(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL
        )));
    }

    #[test]
    fn ctrl_q_is_quit() {
        assert!(is_quit(&make_key(
            KeyCode::Char('q'),
            KeyModifiers::CONTROL
        )));
    }

    #[test]
    fn shift_q_not_quit() {
        assert!(!is_quit(&make_key(
            KeyCode::Char('Q'),
            KeyModifiers::SHIFT
        )));
    }

    #[test]
    fn random_key_not_quit() {
        assert!(!is_quit(&make_key(KeyCode::Char('x'), KeyModifiers::NONE)));
    }

    #[test]
    fn esc_not_quit() {
        assert!(!is_quit(&make_key(KeyCode::Esc, KeyModifiers::NONE)));
    }

    // -- is_run_query --

    #[test]
    fn ctrl_s_is_run_query() {
        assert!(is_run_query(&make_key(
            KeyCode::Char('s'),
            KeyModifiers::CONTROL
        )));
    }

    #[test]
    fn f5_is_run_query() {
        assert!(is_run_query(&make_key(KeyCode::F(5), KeyModifiers::NONE)));
    }

    #[test]
    fn enter_not_run_query() {
        assert!(!is_run_query(&make_key(KeyCode::Enter, KeyModifiers::NONE)));
    }

    #[test]
    fn s_without_ctrl_not_run_query() {
        assert!(!is_run_query(&make_key(
            KeyCode::Char('s'),
            KeyModifiers::NONE
        )));
    }

    // -- is_commit --

    #[test]
    fn ctrl_w_is_commit() {
        assert!(is_commit(&make_key(
            KeyCode::Char('w'),
            KeyModifiers::CONTROL
        )));
    }

    #[test]
    fn w_without_ctrl_not_commit() {
        assert!(!is_commit(&make_key(
            KeyCode::Char('w'),
            KeyModifiers::NONE
        )));
    }

    #[test]
    fn ctrl_s_not_commit() {
        assert!(!is_commit(&make_key(
            KeyCode::Char('s'),
            KeyModifiers::CONTROL
        )));
    }
}
