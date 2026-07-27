use ratatui::style::Style;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

use crate::ui::theme;

/// Ordered list of highlight names we recognise.
///
/// The index into this array is the `Highlight.0` value returned by
/// `tree_sitter_highlight`.  The order **must** match what we pass to
/// `HighlightConfiguration::configure()`.
const HIGHLIGHT_NAMES: &[&str] = &[
    "keyword",
    "function.call",
    "function",
    "string",
    "type",
    "type.builtin",
    "number",
    "float",
    "boolean",
    "comment",
    "operator",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "variable",
    "field",
    "parameter",
    "conditional",
    "attribute",
    "storageclass",
    "constant",
    "constant.builtin",
];

/// The source split into lines, all in one colour.
///
/// What the editor falls back to whenever highlighting is unavailable: no
/// grammar, or a parse that failed on this particular text.
fn unstyled(source: &str, style: Style) -> Vec<Vec<(Style, String)>> {
    source
        .lines()
        .map(|line| vec![(style, line.to_string())])
        .collect()
}

/// Map a highlight-name index to a Catppuccin Mocha foreground colour.
fn style_for_highlight(idx: usize) -> Style {
    let fg = match HIGHLIGHT_NAMES.get(idx) {
        Some(&"keyword") => theme::MAUVE,
        Some(&"function.call" | &"function") => theme::BLUE,
        Some(&"string") => theme::GREEN,
        Some(&"type" | &"type.builtin") => theme::YELLOW,
        Some(&"number" | &"float") => theme::PEACH,
        Some(&"boolean") => theme::PEACH,
        Some(&"comment") => theme::OVERLAY1,
        Some(&"operator") => theme::RED,
        Some(&"punctuation" | &"punctuation.bracket" | &"punctuation.delimiter") => theme::SURFACE2,
        Some(&"variable") => theme::FLAMINGO,
        Some(&"field" | &"parameter") => theme::SAPPHIRE,
        Some(&"conditional") => theme::LAVENDER,
        Some(&"attribute" | &"storageclass") => theme::TEAL,
        Some(&"constant" | &"constant.builtin") => theme::PEACH,
        _ => theme::TEXT,
    };
    Style::default().fg(fg)
}

pub struct SqlHighlighter {
    highlighter: Highlighter,
    /// `None` if the grammar would not load.
    ///
    /// The editor then shows plain, unstyled SQL. That is worse-looking and
    /// entirely usable, which is more than can be said for the `expect` this
    /// replaces: it ran during startup, inside the alternate screen, and took
    /// the whole application down over a colour scheme.
    config: Option<HighlightConfiguration>,
}

impl SqlHighlighter {
    pub fn new() -> Self {
        let config = match HighlightConfiguration::new(
            tree_sitter_sequel::LANGUAGE.into(),
            "sql",
            tree_sitter_sequel::HIGHLIGHTS_QUERY,
            "", // no injection query
            "", // no locals query
        ) {
            Ok(mut config) => {
                config.configure(HIGHLIGHT_NAMES);
                Some(config)
            }
            Err(e) => {
                tracing::error!("SQL highlighting disabled, grammar failed to load: {e}");
                None
            }
        };

        Self {
            highlighter: Highlighter::new(),
            config,
        }
    }

    /// Tokenise `source` and return per-line styled segments.
    ///
    /// Each inner `Vec` corresponds to one line of the source text.
    /// Every `(Style, String)` pair represents a contiguous run of
    /// identically-styled characters within that line.
    pub fn highlight_lines(&mut self, source: &str) -> Vec<Vec<(Style, String)>> {
        let num_lines = source.lines().count().max(1);
        let mut lines: Vec<Vec<(Style, String)>> = Vec::with_capacity(num_lines);
        lines.push(Vec::new());

        let default_style = Style::default().fg(theme::TEXT);

        // No grammar: the editor still shows the SQL, just without colour.
        let Some(config) = self.config.as_ref() else {
            return unstyled(source, default_style);
        };

        let events = match self
            .highlighter
            .highlight(config, source.as_bytes(), None, |_| None)
        {
            Ok(iter) => iter,
            Err(_) => return unstyled(source, default_style),
        };

        let mut style_stack: Vec<Style> = vec![default_style];

        for event in events {
            match event {
                Ok(HighlightEvent::Source { start, end }) => {
                    let style = *style_stack.last().unwrap_or(&default_style);
                    let slice = &source[start..end];

                    // Split by newlines so each segment stays on the correct line.
                    for (i, part) in slice.split('\n').enumerate() {
                        if i > 0 {
                            lines.push(Vec::new());
                        }
                        // `lines` is seeded with one entry and only ever grows,
                        // so this always matches — but a panic in a draw path
                        // costs the whole frame, and skipping a span does not.
                        if let (false, Some(line)) = (part.is_empty(), lines.last_mut()) {
                            line.push((style, part.to_string()));
                        }
                    }
                }
                Ok(HighlightEvent::HighlightStart(h)) => {
                    style_stack.push(style_for_highlight(h.0));
                }
                Ok(HighlightEvent::HighlightEnd) => {
                    if style_stack.len() > 1 {
                        style_stack.pop();
                    }
                }
                Err(_) => {}
            }
        }

        // Ensure we always have at least as many lines as the source.
        // (Trailing newline means the last line may be empty.)
        if source.ends_with('\n') && lines.len() < num_lines + 1 {
            lines.push(Vec::new());
        }

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_select_star() {
        let mut hl = SqlHighlighter::new();
        let lines = hl.highlight_lines("SELECT * FROM users");
        assert!(!lines.is_empty());
        // The first line should have at least one segment
        assert!(!lines[0].is_empty());
        // Reconstruct full text
        let text: String = lines[0].iter().map(|(_, s)| s.as_str()).collect();
        assert_eq!(text, "SELECT * FROM users");
    }

    #[test]
    fn highlight_multiline() {
        let mut hl = SqlHighlighter::new();
        let src = "SELECT id\nFROM users\nWHERE active = true";
        let lines = hl.highlight_lines(src);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn highlight_empty() {
        let mut hl = SqlHighlighter::new();
        let lines = hl.highlight_lines("");
        assert!(!lines.is_empty());
    }
}
