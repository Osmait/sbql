//! SQL AST manipulation for data pushdown.
//!
//! Uses `sqlparser-rs` to parse and modify the user's SQL so that ORDER BY
//! and WHERE clauses are injected at the database level rather than sorting
//! or filtering in Rust memory.
//!
//! Strategy:
//!   1. Parse the SQL into an AST.
//!   2. Find the outermost `SELECT` statement.
//!   3. Inject / replace the relevant clause.
//!   4. Re-serialize back to a SQL string.
//!   5. On parse failure fall back to a safe subquery wrapper.

use sqlparser::ast::{Expr, Ident, OrderByExpr, Query, SetExpr, Statement};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use crate::error::{Result, SbqlError};

/// Direction for column ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Inject (or replace) an `ORDER BY <column> ASC/DESC` clause into `sql`.
/// If the SQL cannot be parsed, wraps it in a subquery.
pub fn apply_order(sql: &str, column: &str, direction: SortDirection) -> Result<String> {
    match parse_single_select(sql) {
        Ok(mut query) => {
            let order_expr = OrderByExpr {
                expr: Expr::Identifier(Ident::new(column)),
                asc: Some(direction == SortDirection::Ascending),
                nulls_first: None,
                with_fill: None,
            };
            query.order_by = Some(sqlparser::ast::OrderBy {
                exprs: vec![order_expr],
                interpolate: None,
            });
            Ok(query.to_string())
        }
        Err(_) => {
            // Fallback: wrap in a subquery
            let dir = if direction == SortDirection::Ascending {
                "ASC"
            } else {
                "DESC"
            };
            let trimmed = sql.trim_end_matches(';').trim();
            Ok(format!(
                "SELECT * FROM ({trimmed}) AS _sbql_order ORDER BY {column} {dir}"
            ))
        }
    }
}

/// Remove the `ORDER BY` clause from `sql`.
pub fn clear_order(sql: &str) -> Result<String> {
    match parse_single_select(sql) {
        Ok(mut query) => {
            query.order_by = None;
            Ok(query.to_string())
        }
        Err(_) => Ok(sql.to_owned()),
    }
}

/// Inject a filter into `sql`.
///
/// `filter_query` format:
/// - `"col:value"` → `WHERE col ILIKE '%value%'`
/// - `"plain text"` → adds an `OR` ILIKE for every provided column.
///
/// When `columns` is provided (filled in by the TUI after the first query
/// execution) the global text search generates per-column ILIKE conditions.
pub fn apply_filter(sql: &str, filter_query: &str, columns: Option<&[String]>) -> Result<String> {
    let (col_opt, value) = parse_filter_query(filter_query);

    // Keep filtering robust across mixed column types by always casting to text.
    let trimmed = sql.trim_end_matches(';').trim();
    let escaped = value.replace('\'', "''");
    if let Some(col) = col_opt {
        let col = quote_ident(&col);
        Ok(format!(
            "SELECT * FROM ({trimmed}) AS _sbql_filter WHERE CAST(_sbql_filter.{col} AS TEXT) ILIKE '%{escaped}%'"
        ))
    } else {
        match columns {
            Some(cols) if !cols.is_empty() => {
                let ors = cols
                    .iter()
                    .map(|c| {
                        let c = quote_ident(c);
                        format!("CAST(_sbql_filter.{c} AS TEXT) ILIKE '%{escaped}%'")
                    })
                    .collect::<Vec<_>>()
                    .join(" OR ");
                Ok(format!(
                    "SELECT * FROM ({trimmed}) AS _sbql_filter WHERE {ors}"
                ))
            }
            _ => Ok(format!(
                "SELECT * FROM ({trimmed}) AS _sbql_filter WHERE CAST(_sbql_filter.* AS TEXT) ILIKE '%{escaped}%'"
            )),
        }
    }
}

/// Remove any injected WHERE filter, leaving the base query intact.
/// NOTE: This clears the entire WHERE clause. Callers should store the
/// original SQL and reapply only the desired modifiers.
#[allow(dead_code)]
pub fn clear_filter(sql: &str) -> Result<String> {
    match parse_single_select(sql) {
        Ok(mut query) => {
            if let SetExpr::Select(ref mut sel) = *query.body {
                sel.selection = None;
            }
            Ok(query.to_string())
        }
        Err(_) => Ok(sql.to_owned()),
    }
}

/// Build a minimal `SELECT * FROM <schema>.<table>` query.
pub fn table_select_sql(schema: &str, table: &str) -> String {
    format!("SELECT * FROM \"{schema}\".\"{table}\"")
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse `sql` and return the inner `Query` node if it is a single SELECT.
fn parse_single_select(sql: &str) -> Result<Box<Query>> {
    let dialect = PostgreSqlDialect {};
    let trimmed = sql.trim_end_matches(';').trim();
    let mut stmts =
        Parser::parse_sql(&dialect, trimmed).map_err(|e| SbqlError::SqlParse(e.to_string()))?;

    if stmts.len() != 1 {
        return Err(SbqlError::SqlParse(
            "Expected exactly one SQL statement".into(),
        ));
    }

    match stmts.remove(0) {
        Statement::Query(q) => Ok(q),
        _ => Err(SbqlError::SqlParse(
            "Statement is not a SELECT query".into(),
        )),
    }
}

/// Parse `"col:value"` or `"plain text"`.
fn parse_filter_query(q: &str) -> (Option<String>, &str) {
    if let Some(colon_pos) = q.find(':') {
        let col = q[..colon_pos].trim().to_owned();
        let val = q[colon_pos + 1..].trim();
        if !col.is_empty() && !col.contains(' ') {
            return (Some(col), val);
        }
    }
    (None, q)
}

fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_order_asc() {
        let sql = "SELECT * FROM users";
        let result = apply_order(sql, "name", SortDirection::Ascending).unwrap();
        let upper = result.to_uppercase();
        assert!(upper.contains("ORDER BY"), "missing ORDER BY: {result}");
        assert!(upper.contains("NAME"), "missing column: {result}");
        assert!(upper.contains("ASC"), "missing ASC: {result}");
    }

    #[test]
    fn test_apply_order_desc() {
        let sql = "SELECT id, name FROM users WHERE active = true";
        let result = apply_order(sql, "created_at", SortDirection::Descending).unwrap();
        let upper = result.to_uppercase();
        assert!(upper.contains("ORDER BY"));
        assert!(upper.contains("CREATED_AT"));
        assert!(upper.contains("DESC"));
    }

    #[test]
    fn test_clear_order() {
        let sql = "SELECT * FROM users ORDER BY name ASC";
        let result = clear_order(sql).unwrap();
        assert!(!result.to_uppercase().contains("ORDER BY"));
    }

    #[test]
    fn test_apply_filter_column() {
        let sql = "SELECT * FROM users";
        let result = apply_filter(sql, "status:active", None).unwrap();
        let upper = result.to_uppercase();
        assert!(upper.contains("WHERE"), "missing WHERE: {result}");
        assert!(upper.contains("ILIKE"), "missing ILIKE: {result}");
        assert!(upper.contains("%ACTIVE%"), "missing value: {result}");
    }

    #[test]
    fn test_apply_filter_global() {
        let sql = "SELECT * FROM users";
        let cols = vec!["name".to_string(), "email".to_string()];
        let result = apply_filter(sql, "alice", Some(&cols)).unwrap();
        let upper = result.to_uppercase();
        assert!(upper.contains("WHERE"));
        assert!(upper.contains("ILIKE"));
        assert!(upper.contains("NAME") || upper.contains("EMAIL"));
    }

    #[test]
    fn test_apply_order_replaces_existing() {
        let sql = "SELECT * FROM users ORDER BY id ASC";
        let result = apply_order(sql, "email", SortDirection::Descending).unwrap();
        let upper = result.to_uppercase();
        assert!(upper.contains("EMAIL"));
        assert!(!upper.contains("ORDER BY ID"));
    }

    #[test]
    fn test_apply_order_fallback() {
        // Invalid syntax for single SELECT or complex query
        let sql = "INVALID SQL STATEMENT";
        let result = apply_order(sql, "col", SortDirection::Ascending).unwrap();
        assert_eq!(
            result,
            "SELECT * FROM (INVALID SQL STATEMENT) AS _sbql_order ORDER BY col ASC"
        );
    }

    #[test]
    fn test_clear_filter() {
        let sql = "SELECT * FROM users WHERE status = 'active'";
        let result = clear_filter(sql).unwrap();
        let upper = result.to_uppercase();
        assert!(!upper.contains("WHERE"));
        assert!(upper.contains("SELECT * FROM USERS"));
    }

    #[test]
    fn test_table_select_sql() {
        assert_eq!(
            table_select_sql("public", "users"),
            "SELECT * FROM \"public\".\"users\""
        );
    }

    #[test]
    fn test_parse_filter_query() {
        assert_eq!(
            parse_filter_query("col:val"),
            (Some("col".to_string()), "val")
        );
        assert_eq!(
            parse_filter_query("status: active"),
            (Some("status".to_string()), "active")
        );
        assert_eq!(
            parse_filter_query("invalid col:val"),
            (None, "invalid col:val")
        ); // spaces not allowed in column name
        assert_eq!(parse_filter_query("plain text"), (None, "plain text"));
    }

    #[test]
    fn test_apply_filter_fallback() {
        // Complex query fallback
        let sql = "SELECT * FROM users UNION SELECT * FROM admins";
        let result = apply_filter(sql, "test", None).unwrap();
        assert!(result.starts_with(
            "SELECT * FROM (SELECT * FROM users UNION SELECT * FROM admins) AS _sbql_filter"
        ));
        assert!(result.contains("WHERE CAST(_sbql_filter.* AS TEXT) ILIKE '%test%'"));
    }
}
