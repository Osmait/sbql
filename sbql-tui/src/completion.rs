use sbql_core::{DiagramData, TableEntry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Table,
    Column,
    Keyword,
}

#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub text: String,
    pub detail: String,
    pub kind: CompletionKind,
}

#[derive(Debug, Default)]
pub struct CompletionState {
    pub visible: bool,
    pub items: Vec<CompletionItem>,
    pub selected: usize,
    pub prefix: String,
}

impl CompletionState {
    pub fn dismiss(&mut self) {
        self.visible = false;
        self.items.clear();
        self.selected = 0;
        self.prefix.clear();
    }

    pub fn move_up(&mut self) {
        if !self.items.is_empty() {
            self.selected = if self.selected == 0 {
                self.items.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn move_down(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + 1) % self.items.len();
        }
    }

    pub fn selected_item(&self) -> Option<&CompletionItem> {
        self.items.get(self.selected)
    }
}

/// SQL keywords offered by autocomplete.
pub const SQL_KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "AND",
    "OR",
    "NOT",
    "IN",
    "IS",
    "NULL",
    "AS",
    "ON",
    "JOIN",
    "INNER",
    "LEFT",
    "RIGHT",
    "OUTER",
    "FULL",
    "CROSS",
    "GROUP",
    "BY",
    "ORDER",
    "ASC",
    "DESC",
    "HAVING",
    "LIMIT",
    "OFFSET",
    "INSERT",
    "INTO",
    "VALUES",
    "UPDATE",
    "SET",
    "DELETE",
    "CREATE",
    "TABLE",
    "ALTER",
    "DROP",
    "INDEX",
    "VIEW",
    "DISTINCT",
    "ALL",
    "EXISTS",
    "BETWEEN",
    "LIKE",
    "ILIKE",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "CAST",
    "UNION",
    "INTERSECT",
    "EXCEPT",
    "WITH",
    "RECURSIVE",
    "RETURNING",
    "PRIMARY",
    "KEY",
    "FOREIGN",
    "REFERENCES",
    "CONSTRAINT",
    "UNIQUE",
    "CHECK",
    "DEFAULT",
    "CASCADE",
    "RESTRICT",
    "TRUE",
    "FALSE",
    "COUNT",
    "SUM",
    "AVG",
    "MIN",
    "MAX",
    "COALESCE",
    "NULLIF",
];

/// Walk backwards from the cursor to find the current word prefix.
pub fn extract_prefix(lines: &[String], row: usize, col: usize) -> String {
    if row >= lines.len() {
        return String::new();
    }
    let line = &lines[row];
    let bytes = line.as_bytes();
    let end = col.min(bytes.len());
    let mut start = end;
    while start > 0 {
        let ch = bytes[start - 1] as char;
        if ch.is_alphanumeric() || ch == '_' || ch == '.' {
            start -= 1;
        } else {
            break;
        }
    }
    line[start..end].to_string()
}

/// Compute completion candidates for the given prefix.
pub fn compute_completions(
    prefix: &str,
    tables: &[TableEntry],
    diagram: Option<&DiagramData>,
) -> Vec<CompletionItem> {
    if prefix.is_empty() {
        return Vec::new();
    }

    let lower = prefix.to_ascii_lowercase();
    let mut items = Vec::new();

    // 1. Table names
    for t in tables {
        let name = &t.name;
        if name.to_ascii_lowercase().starts_with(&lower) {
            items.push(CompletionItem {
                text: name.clone(),
                detail: t.schema.clone(),
                kind: CompletionKind::Table,
            });
        }
    }

    // 2. Column names (from diagram data, deduplicated)
    if let Some(diag) = diagram {
        let mut seen = std::collections::HashSet::new();
        for ts in &diag.tables {
            for col in &ts.columns {
                if col.name.to_ascii_lowercase().starts_with(&lower) && seen.insert(&col.name) {
                    items.push(CompletionItem {
                        text: col.name.clone(),
                        detail: format!("{}.{}", ts.name, col.data_type),
                        kind: CompletionKind::Column,
                    });
                }
            }
        }
    }

    // 3. SQL keywords
    for &kw in SQL_KEYWORDS {
        if kw.to_ascii_lowercase().starts_with(&lower) {
            items.push(CompletionItem {
                text: kw.to_string(),
                detail: String::new(),
                kind: CompletionKind::Keyword,
            });
        }
    }

    // Sort: tables first, then columns, then keywords. Shorter first within group.
    items.sort_by(|a, b| {
        a.kind
            .cmp_order()
            .cmp(&b.kind.cmp_order())
            .then_with(|| a.text.len().cmp(&b.text.len()))
            .then_with(|| a.text.cmp(&b.text))
    });

    items.truncate(10);
    items
}

impl CompletionKind {
    fn cmp_order(self) -> u8 {
        match self {
            CompletionKind::Table => 0,
            CompletionKind::Column => 1,
            CompletionKind::Keyword => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_prefix_basic() {
        let lines = vec!["SELECT * FROM us".to_string()];
        assert_eq!(extract_prefix(&lines, 0, 16), "us");
    }

    #[test]
    fn extract_prefix_start_of_line() {
        let lines = vec!["SEL".to_string()];
        assert_eq!(extract_prefix(&lines, 0, 3), "SEL");
    }

    #[test]
    fn extract_prefix_after_space() {
        let lines = vec!["SELECT ".to_string()];
        assert_eq!(extract_prefix(&lines, 0, 7), "");
    }

    #[test]
    fn compute_completions_keywords() {
        let items = compute_completions("SEL", &[], None);
        assert!(!items.is_empty());
        assert!(items.iter().any(|i| i.text == "SELECT"));
    }

    #[test]
    fn compute_completions_tables() {
        let tables = vec![
            TableEntry {
                schema: "public".into(),
                name: "users".into(),
            },
            TableEntry {
                schema: "public".into(),
                name: "posts".into(),
            },
        ];
        let items = compute_completions("us", &tables, None);
        assert!(items.iter().any(|i| i.text == "users"));
    }

    #[test]
    fn compute_completions_empty_prefix() {
        let items = compute_completions("", &[], None);
        assert!(items.is_empty());
    }

    #[test]
    fn compute_completions_limit() {
        // Keyword prefix "S" matches many keywords, result should be capped at 10
        let items = compute_completions("S", &[], None);
        assert!(items.len() <= 10);
    }
}
