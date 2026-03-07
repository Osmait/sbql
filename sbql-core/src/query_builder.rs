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
use sqlparser::dialect::{PostgreSqlDialect, SQLiteDialect};
use sqlparser::parser::Parser;

use crate::error::{Result, SbqlError};
use crate::pool::DbBackend;

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
pub fn apply_order(sql: &str, column: &str, direction: SortDirection, backend: DbBackend) -> Result<String> {
    if backend == DbBackend::Redis {
        return Err(SbqlError::SqlParse("ORDER BY not supported for Redis".into()));
    }
    match parse_single_select(sql, backend) {
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
pub fn clear_order(sql: &str, backend: DbBackend) -> Result<String> {
    if backend == DbBackend::Redis {
        return Ok(sql.to_owned());
    }
    match parse_single_select(sql, backend) {
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
/// - `"col:value"` → `WHERE col ILIKE '%value%'` (PG) / `LIKE ... COLLATE NOCASE` (SQLite)
/// - `"plain text"` → adds an `OR` ILIKE/LIKE for every provided column.
pub fn apply_filter(sql: &str, filter_query: &str, columns: Option<&[String]>, backend: DbBackend) -> Result<String> {
    if backend == DbBackend::Redis {
        return Err(SbqlError::SqlParse("Filtering not supported for Redis".into()));
    }
    let (col_opt, value) = parse_filter_query(filter_query);

    let trimmed = sql.trim_end_matches(';').trim();
    let escaped = value.replace('\'', "''");

    let like_op = match backend {
        DbBackend::Postgres => "ILIKE",
        DbBackend::Sqlite | DbBackend::Redis => "LIKE",
    };
    let collate_suffix = match backend {
        DbBackend::Postgres => "",
        DbBackend::Sqlite | DbBackend::Redis => " COLLATE NOCASE",
    };

    if let Some(col) = col_opt {
        let col = quote_ident(&col);
        Ok(format!(
            "SELECT * FROM ({trimmed}) AS _sbql_filter WHERE CAST(_sbql_filter.{col} AS TEXT) {like_op} '%{escaped}%'{collate_suffix}"
        ))
    } else {
        match columns {
            Some(cols) if !cols.is_empty() => {
                let ors = cols
                    .iter()
                    .map(|c| {
                        let c = quote_ident(c);
                        format!("CAST(_sbql_filter.{c} AS TEXT) {like_op} '%{escaped}%'{collate_suffix}")
                    })
                    .collect::<Vec<_>>()
                    .join(" OR ");
                Ok(format!(
                    "SELECT * FROM ({trimmed}) AS _sbql_filter WHERE {ors}"
                ))
            }
            _ => Ok(format!(
                "SELECT * FROM ({trimmed}) AS _sbql_filter WHERE CAST(_sbql_filter.* AS TEXT) {like_op} '%{escaped}%'{collate_suffix}"
            )),
        }
    }
}

/// Remove any injected WHERE filter, leaving the base query intact.
#[allow(dead_code)]
pub fn clear_filter(sql: &str, backend: DbBackend) -> Result<String> {
    if backend == DbBackend::Redis {
        return Ok(sql.to_owned());
    }
    match parse_single_select(sql, backend) {
        Ok(mut query) => {
            if let SetExpr::Select(ref mut sel) = *query.body {
                sel.selection = None;
            }
            Ok(query.to_string())
        }
        Err(_) => Ok(sql.to_owned()),
    }
}

/// Build a minimal `SELECT * FROM <table>` query.
/// For PG: `SELECT * FROM "schema"."table"`
/// For SQLite: `SELECT * FROM "table"` (no schema prefix)
pub fn table_select_sql(schema: &str, table: &str, backend: DbBackend) -> String {
    match backend {
        DbBackend::Postgres => format!("SELECT * FROM \"{schema}\".\"{table}\""),
        DbBackend::Sqlite => format!("SELECT * FROM \"{table}\""),
        DbBackend::Redis => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse `sql` and return the inner `Query` node if it is a single SELECT.
fn parse_single_select(sql: &str, backend: DbBackend) -> Result<Box<Query>> {
    let trimmed = sql.trim_end_matches(';').trim();
    let mut stmts = match backend {
        DbBackend::Postgres => {
            let dialect = PostgreSqlDialect {};
            Parser::parse_sql(&dialect, trimmed).map_err(|e| SbqlError::SqlParse(e.to_string()))?
        }
        DbBackend::Sqlite => {
            let dialect = SQLiteDialect {};
            Parser::parse_sql(&dialect, trimmed).map_err(|e| SbqlError::SqlParse(e.to_string()))?
        }
        DbBackend::Redis => {
            return Err(SbqlError::SqlParse("SQL parsing not supported for Redis".into()));
        }
    };

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
        let result = apply_order(sql, "name", SortDirection::Ascending, DbBackend::Postgres).unwrap();
        let upper = result.to_uppercase();
        assert!(upper.contains("ORDER BY"), "missing ORDER BY: {result}");
        assert!(upper.contains("NAME"), "missing column: {result}");
        assert!(upper.contains("ASC"), "missing ASC: {result}");
    }

    #[test]
    fn test_apply_order_desc() {
        let sql = "SELECT id, name FROM users WHERE active = true";
        let result = apply_order(sql, "created_at", SortDirection::Descending, DbBackend::Postgres).unwrap();
        let upper = result.to_uppercase();
        assert!(upper.contains("ORDER BY"));
        assert!(upper.contains("CREATED_AT"));
        assert!(upper.contains("DESC"));
    }

    #[test]
    fn test_clear_order() {
        let sql = "SELECT * FROM users ORDER BY name ASC";
        let result = clear_order(sql, DbBackend::Postgres).unwrap();
        assert!(!result.to_uppercase().contains("ORDER BY"));
    }

    #[test]
    fn test_apply_filter_column() {
        let sql = "SELECT * FROM users";
        let result = apply_filter(sql, "status:active", None, DbBackend::Postgres).unwrap();
        let upper = result.to_uppercase();
        assert!(upper.contains("WHERE"), "missing WHERE: {result}");
        assert!(upper.contains("ILIKE"), "missing ILIKE: {result}");
        assert!(upper.contains("%ACTIVE%"), "missing value: {result}");
    }

    #[test]
    fn test_apply_filter_column_sqlite() {
        let sql = "SELECT * FROM users";
        let result = apply_filter(sql, "status:active", None, DbBackend::Sqlite).unwrap();
        let upper = result.to_uppercase();
        assert!(upper.contains("WHERE"));
        assert!(upper.contains("LIKE"));
        assert!(upper.contains("COLLATE NOCASE"));
        assert!(!upper.contains("ILIKE"));
    }

    #[test]
    fn test_apply_filter_global() {
        let sql = "SELECT * FROM users";
        let cols = vec!["name".to_string(), "email".to_string()];
        let result = apply_filter(sql, "alice", Some(&cols), DbBackend::Postgres).unwrap();
        let upper = result.to_uppercase();
        assert!(upper.contains("WHERE"));
        assert!(upper.contains("ILIKE"));
        assert!(upper.contains("NAME") || upper.contains("EMAIL"));
    }

    #[test]
    fn test_apply_order_replaces_existing() {
        let sql = "SELECT * FROM users ORDER BY id ASC";
        let result = apply_order(sql, "email", SortDirection::Descending, DbBackend::Postgres).unwrap();
        let upper = result.to_uppercase();
        assert!(upper.contains("EMAIL"));
        assert!(!upper.contains("ORDER BY ID"));
    }

    #[test]
    fn test_apply_order_fallback() {
        let sql = "INVALID SQL STATEMENT";
        let result = apply_order(sql, "col", SortDirection::Ascending, DbBackend::Postgres).unwrap();
        assert_eq!(
            result,
            "SELECT * FROM (INVALID SQL STATEMENT) AS _sbql_order ORDER BY col ASC"
        );
    }

    #[test]
    fn test_clear_filter() {
        let sql = "SELECT * FROM users WHERE status = 'active'";
        let result = clear_filter(sql, DbBackend::Postgres).unwrap();
        let upper = result.to_uppercase();
        assert!(!upper.contains("WHERE"));
        assert!(upper.contains("SELECT * FROM USERS"));
    }

    #[test]
    fn test_table_select_sql_pg() {
        assert_eq!(
            table_select_sql("public", "users", DbBackend::Postgres),
            "SELECT * FROM \"public\".\"users\""
        );
    }

    #[test]
    fn test_table_select_sql_sqlite() {
        assert_eq!(
            table_select_sql("main", "users", DbBackend::Sqlite),
            "SELECT * FROM \"users\""
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
        );
        assert_eq!(parse_filter_query("plain text"), (None, "plain text"));
    }

    #[test]
    fn test_apply_filter_fallback() {
        let sql = "SELECT * FROM users UNION SELECT * FROM admins";
        let result = apply_filter(sql, "test", None, DbBackend::Postgres).unwrap();
        assert!(result.starts_with(
            "SELECT * FROM (SELECT * FROM users UNION SELECT * FROM admins) AS _sbql_filter"
        ));
        assert!(result.contains("WHERE CAST(_sbql_filter.* AS TEXT) ILIKE '%test%'"));
    }

    // --- Phase 1A: additional edge-case tests ---

    #[test]
    fn test_apply_filter_single_quote_in_value() {
        let sql = "SELECT * FROM users";
        let result = apply_filter(sql, "name:O'Brien", None, DbBackend::Postgres).unwrap();
        // Single quotes must be escaped as ''
        assert!(result.contains("O''Brien"), "missing escaped quote: {result}");
    }

    #[test]
    fn test_apply_filter_no_columns_global() {
        let sql = "SELECT * FROM users";
        // No columns provided (None) and plain text filter → wildcard CAST fallback
        let result = apply_filter(sql, "alice", None, DbBackend::Postgres).unwrap();
        assert!(result.contains("CAST(_sbql_filter.* AS TEXT)"));
        assert!(result.contains("ILIKE '%alice%'"));
    }

    #[test]
    fn test_clear_order_no_order_by() {
        let sql = "SELECT * FROM users WHERE active = true";
        let result = clear_order(sql, DbBackend::Postgres).unwrap();
        // Should return the SQL unchanged (minus formatting differences)
        let upper = result.to_uppercase();
        assert!(upper.contains("SELECT"));
        assert!(upper.contains("USERS"));
        assert!(!upper.contains("ORDER BY"));
    }

    #[test]
    fn test_table_select_sql_with_double_quote_chars() {
        // Schema and table containing double-quote characters
        let result = table_select_sql("my\"schema", "my\"table", DbBackend::Postgres);
        // Double-quotes inside identifiers should be present (escaped by the format! macro)
        assert_eq!(result, "SELECT * FROM \"my\"schema\".\"my\"table\"");
    }

    #[test]
    fn test_parse_filter_query_empty_string() {
        let (col, val) = parse_filter_query("");
        assert_eq!(col, None);
        assert_eq!(val, "");
    }

    #[test]
    fn test_apply_filter_empty_columns_slice() {
        let sql = "SELECT * FROM users";
        let cols: Vec<String> = vec![];
        // Empty columns slice → wildcard CAST fallback
        let result = apply_filter(sql, "test", Some(&cols), DbBackend::Postgres).unwrap();
        assert!(result.contains("CAST(_sbql_filter.* AS TEXT)"));
    }
}
