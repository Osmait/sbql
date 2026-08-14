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

use sqlparser::ast::{Expr, Ident, Offset, OffsetRows, OrderByExpr, Query, Statement, Value};
use sqlparser::dialect::{MsSqlDialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect};
use sqlparser::parser::Parser;

use crate::error::{Result, SbqlError};
use crate::pool::DbBackend;
use crate::sql_util::{quote_ident, quote_ident_mysql, quote_ident_sqlserver};

/// The quote character each backend uses for identifiers, for building a
/// [`sqlparser`] `Ident` that round-trips quoted. SQL Server's `[...]`
/// brackets are not a single symmetric quote char, so it borrows the ANSI
/// double quote, which SQL Server also accepts for identifiers.
fn ident_quote_char(backend: DbBackend) -> char {
    match backend {
        DbBackend::Mysql => '`',
        _ => '"',
    }
}

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
#[tracing::instrument(skip_all, fields(column, direction = ?direction, backend = ?backend))]
pub fn apply_order(
    sql: &str,
    column: &str,
    direction: SortDirection,
    backend: DbBackend,
) -> Result<String> {
    if backend == DbBackend::Redis
        || backend == DbBackend::DynamoDb
        || backend == DbBackend::MongoDb
    {
        return Err(SbqlError::SqlParse(
            "ORDER BY not supported for this backend".into(),
        ));
    }
    // SQL Server supports ORDER BY via the same path as Postgres/MySQL
    match parse_single_select(sql, backend) {
        Ok(mut query) => {
            // Quote the identifier so a mixed-case (`createdAt`) or reserved
            // column name is not folded or misparsed. A bare `Ident::new`
            // serialized unquoted, silently sorting by the wrong thing.
            let order_expr = OrderByExpr {
                expr: Expr::Identifier(Ident::with_quote(ident_quote_char(backend), column)),
                asc: Some(direction == SortDirection::Ascending),
                nulls_first: None,
                with_fill: None,
            };
            query.order_by = Some(sqlparser::ast::OrderBy {
                exprs: vec![order_expr],
                interpolate: None,
            });
            // The paginator wraps this SQL as a derived table, and SQL Server
            // rejects ORDER BY in a subquery unless it also has OFFSET/TOP.
            // `OFFSET 0 ROWS` makes it legal without dropping any rows.
            if backend == DbBackend::SqlServer {
                query.offset = Some(Offset {
                    value: Expr::Value(Value::Number("0".into(), false)),
                    rows: OffsetRows::Rows,
                });
            }
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
            let safe_col = quote_column(column, backend);
            let offset_suffix = if backend == DbBackend::SqlServer {
                " OFFSET 0 ROWS"
            } else {
                ""
            };
            Ok(format!(
                "SELECT * FROM ({trimmed}) AS _sbql_order ORDER BY {safe_col} {dir}{offset_suffix}"
            ))
        }
    }
}

/// Quote a column identifier for the given backend.
fn quote_column(column: &str, backend: DbBackend) -> String {
    match backend {
        DbBackend::Mysql => quote_ident_mysql(column),
        DbBackend::SqlServer => quote_ident_sqlserver(column),
        _ => quote_ident(column),
    }
}

/// Remove the `ORDER BY` clause from `sql`.
#[tracing::instrument(skip_all, fields(backend = ?backend))]
pub fn clear_order(sql: &str, backend: DbBackend) -> Result<String> {
    if backend == DbBackend::Redis
        || backend == DbBackend::DynamoDb
        || backend == DbBackend::MongoDb
    {
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
#[tracing::instrument(skip_all, fields(backend = ?backend))]
pub fn apply_filter(
    sql: &str,
    filter_query: &str,
    columns: Option<&[String]>,
    backend: DbBackend,
) -> Result<String> {
    if backend == DbBackend::Redis
        || backend == DbBackend::DynamoDb
        || backend == DbBackend::MongoDb
    {
        return Err(SbqlError::SqlParse(
            "Filtering not supported for this backend".into(),
        ));
    }
    let (col_opt, value) = parse_filter_query(filter_query);

    let trimmed = sql.trim_end_matches(';').trim();
    // A typed `%` or `_` must match itself, not act as a LIKE wildcard, so the
    // metacharacters are escaped and every clause declares `ESCAPE '\'`.
    // Backslash is escaped first so it cannot combine with a following char.
    let escaped = value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
        .replace('\'', "''");

    let like_op = match backend {
        DbBackend::Postgres => "ILIKE",
        DbBackend::Mysql | DbBackend::SqlServer => "LIKE",
        DbBackend::Sqlite | DbBackend::Redis | DbBackend::DynamoDb | DbBackend::MongoDb => "LIKE",
    };
    let collate_suffix = match backend {
        DbBackend::Postgres | DbBackend::Mysql | DbBackend::SqlServer => "",
        DbBackend::Sqlite | DbBackend::Redis | DbBackend::DynamoDb | DbBackend::MongoDb => {
            " COLLATE NOCASE"
        }
    };
    // COLLATE binds to the pattern; ESCAPE follows it. Ordering the two the
    // other way would attach the collation to the escape char instead and
    // quietly drop case-insensitivity.
    let match_suffix = format!("{collate_suffix} ESCAPE '\\'");
    // Identifier quoting and the CAST target are backend syntax, not
    // preferences: MySQL treats "col" as a string literal (ANSI_QUOTES is off
    // by default) and only casts to CHAR, and SQL Server has neither "col"
    // quoting nor a TEXT cast target. Postgres syntax everywhere meant the
    // filter simply never worked on those backends.
    let (quote, cast_ty): (fn(&str) -> String, &str) = match backend {
        DbBackend::Mysql => (quote_ident_mysql, "CHAR"),
        DbBackend::SqlServer => (quote_ident_sqlserver, "NVARCHAR(MAX)"),
        _ => (quote_ident, "TEXT"),
    };

    if let Some(col) = col_opt {
        let col = quote(&col);
        Ok(format!(
            "SELECT * FROM ({trimmed}) AS _sbql_filter WHERE CAST(_sbql_filter.{col} AS {cast_ty}) {like_op} '%{escaped}%'{match_suffix}"
        ))
    } else {
        match columns {
            Some(cols) if !cols.is_empty() => {
                let mut ors = String::new();
                for (i, c) in cols.iter().enumerate() {
                    if i > 0 {
                        ors.push_str(" OR ");
                    }
                    let c = quote(c);
                    ors.push_str(&format!("CAST(_sbql_filter.{c} AS {cast_ty}) {like_op} '%{escaped}%'{match_suffix}"));
                }
                Ok(format!(
                    "SELECT * FROM ({trimmed}) AS _sbql_filter WHERE {ors}"
                ))
            }
            // Whole-row cast: only Postgres can do this. On the other backends
            // the old wildcard SQL was a guaranteed syntax error at the
            // database, so a clear local error is strictly more honest.
            _ if backend == DbBackend::Postgres => Ok(format!(
                "SELECT * FROM ({trimmed}) AS _sbql_filter WHERE CAST(_sbql_filter.* AS TEXT) {like_op} '%{escaped}%'{match_suffix}"
            )),
            _ => Err(SbqlError::SqlParse(
                "Global filter needs a loaded result on this backend — try col:value".into(),
            )),
        }
    }
}

/// Format SQL by parsing into AST and re-serializing with consistent style.
/// Returns the formatted SQL, or the original if parsing fails.
pub fn format_sql(sql: &str) -> String {
    let trimmed = sql.trim_end_matches(';').trim();
    let dialect = PostgreSqlDialect {};
    match Parser::parse_sql(&dialect, trimmed) {
        Ok(stmts) if !stmts.is_empty() => stmts
            .iter()
            .map(|s| format!("{s}"))
            .collect::<Vec<_>>()
            .join(";\n"),
        _ => {
            // Try generic dialect as fallback
            let dialect = sqlparser::dialect::GenericDialect {};
            match Parser::parse_sql(&dialect, trimmed) {
                Ok(stmts) if !stmts.is_empty() => stmts
                    .iter()
                    .map(|s| format!("{s}"))
                    .collect::<Vec<_>>()
                    .join(";\n"),
                _ => sql.to_owned(),
            }
        }
    }
}

/// Build a minimal `SELECT * FROM <table>` query.
/// For PG: `SELECT * FROM "schema"."table"`
/// For SQLite: `SELECT * FROM "table"` (no schema prefix)
pub fn table_select_sql(schema: &str, table: &str, backend: DbBackend) -> String {
    match backend {
        DbBackend::Postgres => format!(
            "SELECT * FROM {}.{}",
            quote_ident(schema),
            quote_ident(table)
        ),
        DbBackend::Sqlite => format!("SELECT * FROM {}", quote_ident(table)),
        DbBackend::Mysql => format!(
            "SELECT * FROM {}.{}",
            quote_ident_mysql(schema),
            quote_ident_mysql(table)
        ),
        DbBackend::SqlServer => format!(
            "SELECT * FROM {}.{}",
            quote_ident_sqlserver(schema),
            quote_ident_sqlserver(table)
        ),
        DbBackend::Redis | DbBackend::DynamoDb | DbBackend::MongoDb => String::new(),
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
        DbBackend::Mysql => {
            let dialect = MySqlDialect {};
            Parser::parse_sql(&dialect, trimmed).map_err(|e| SbqlError::SqlParse(e.to_string()))?
        }
        DbBackend::SqlServer => {
            let dialect = MsSqlDialect {};
            Parser::parse_sql(&dialect, trimmed).map_err(|e| SbqlError::SqlParse(e.to_string()))?
        }
        DbBackend::Redis | DbBackend::DynamoDb | DbBackend::MongoDb => {
            return Err(SbqlError::SqlParse(
                "SQL parsing not supported for this backend".into(),
            ));
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

/// Parse a filter bar entry into `Some(column)` plus a value, or `None` plus
/// the whole text.
///
/// `"col:value"` is a column filter; `"plain text"`, `":value"` and
/// `"two words:value"` are not, and fall back to matching the text against
/// every column. A column name is not quoted or escaped here, so anything with
/// a space in it cannot be one.
///
/// Public because the TUI must ask this exact question: it decides from the
/// answer whether to offer value suggestions for a column, and offering them
/// for a "column" that [`apply_filter`] will never filter on points the user at
/// results they will not get. One parser, one answer.
pub fn parse_filter_query(q: &str) -> (Option<String>, &str) {
    if let Some(colon_pos) = q.find(':') {
        let col = q[..colon_pos].trim().to_owned();
        let val = q[colon_pos + 1..].trim();
        if !col.is_empty() && !col.contains(' ') {
            return (Some(col), val);
        }
    }
    (None, q)
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
        let result =
            apply_order(sql, "name", SortDirection::Ascending, DbBackend::Postgres).unwrap();
        let upper = result.to_uppercase();
        assert!(upper.contains("ORDER BY"), "missing ORDER BY: {result}");
        assert!(upper.contains("NAME"), "missing column: {result}");
        assert!(upper.contains("ASC"), "missing ASC: {result}");
    }

    #[test]
    fn test_apply_order_desc() {
        let sql = "SELECT id, name FROM users WHERE active = true";
        let result = apply_order(
            sql,
            "created_at",
            SortDirection::Descending,
            DbBackend::Postgres,
        )
        .unwrap();
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
        let result =
            apply_order(sql, "email", SortDirection::Descending, DbBackend::Postgres).unwrap();
        let upper = result.to_uppercase();
        assert!(upper.contains("EMAIL"));
        assert!(!upper.contains("ORDER BY ID"));
    }

    #[test]
    fn test_apply_order_fallback() {
        let sql = "INVALID SQL STATEMENT";
        let result =
            apply_order(sql, "col", SortDirection::Ascending, DbBackend::Postgres).unwrap();
        assert_eq!(
            result,
            "SELECT * FROM (INVALID SQL STATEMENT) AS _sbql_order ORDER BY \"col\" ASC"
        );
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
        assert!(
            result.contains("O''Brien"),
            "missing escaped quote: {result}"
        );
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
        // Schema and table containing double-quote characters — now properly escaped
        let result = table_select_sql("my\"schema", "my\"table", DbBackend::Postgres);
        assert_eq!(result, "SELECT * FROM \"my\"\"schema\".\"my\"\"table\"");
    }

    // --- MySQL tests ---

    #[test]
    fn test_table_select_sql_mysql() {
        assert_eq!(
            table_select_sql("mydb", "users", DbBackend::Mysql),
            "SELECT * FROM `mydb`.`users`"
        );
    }

    #[test]
    fn test_apply_order_mysql() {
        let sql = "SELECT * FROM users";
        let result = apply_order(sql, "name", SortDirection::Ascending, DbBackend::Mysql).unwrap();
        let upper = result.to_uppercase();
        assert!(upper.contains("ORDER BY"), "missing ORDER BY: {result}");
        assert!(upper.contains("NAME"), "missing column: {result}");
        assert!(upper.contains("ASC"), "missing ASC: {result}");
    }

    #[test]
    fn test_clear_order_mysql() {
        let sql = "SELECT * FROM users ORDER BY name ASC";
        let result = clear_order(sql, DbBackend::Mysql).unwrap();
        assert!(!result.to_uppercase().contains("ORDER BY"));
    }

    #[test]
    fn test_apply_filter_column_mysql() {
        let sql = "SELECT * FROM users";
        let result = apply_filter(sql, "status:active", None, DbBackend::Mysql).unwrap();
        let upper = result.to_uppercase();
        assert!(upper.contains("WHERE"), "missing WHERE: {result}");
        assert!(upper.contains("LIKE"), "missing LIKE: {result}");
        // MySQL uses LIKE (case-insensitive by default), not ILIKE
        assert!(
            !upper.contains("ILIKE"),
            "should not use ILIKE for MySQL: {result}"
        );
        // MySQL should NOT have COLLATE NOCASE (that's SQLite)
        assert!(
            !upper.contains("COLLATE NOCASE"),
            "should not use COLLATE NOCASE for MySQL: {result}"
        );
        assert!(upper.contains("%ACTIVE%"), "missing value: {result}");
    }

    #[test]
    fn test_apply_filter_global_mysql() {
        let sql = "SELECT * FROM users";
        let cols = vec!["name".to_string(), "email".to_string()];
        let result = apply_filter(sql, "alice", Some(&cols), DbBackend::Mysql).unwrap();
        let upper = result.to_uppercase();
        assert!(upper.contains("WHERE"));
        assert!(upper.contains("LIKE"));
        assert!(!upper.contains("ILIKE"));
    }

    /// The generated filter must be MySQL syntax end to end: backtick-quoted
    /// identifiers and CAST AS CHAR (MySQL rejects "col" and CAST AS TEXT).
    #[test]
    fn test_apply_filter_mysql_uses_mysql_syntax() {
        let sql = "SELECT * FROM users";
        let result = apply_filter(sql, "status:active", None, DbBackend::Mysql).unwrap();
        assert!(result.contains("`status`"), "not backtick-quoted: {result}");
        assert!(result.contains("AS CHAR)"), "wrong cast target: {result}");
        assert!(
            !result.contains("\"status\""),
            "PG quoting leaked: {result}"
        );
        assert!(!result.contains("AS TEXT"), "PG cast leaked: {result}");

        let cols = vec!["name".to_string()];
        let global = apply_filter(sql, "alice", Some(&cols), DbBackend::Mysql).unwrap();
        assert!(global.contains("`name`"), "{global}");
        assert!(global.contains("AS CHAR)"), "{global}");
    }

    #[test]
    fn test_apply_filter_sqlserver_uses_sqlserver_syntax() {
        let sql = "SELECT * FROM users";
        let result = apply_filter(sql, "status:active", None, DbBackend::SqlServer).unwrap();
        assert!(result.contains("[status]"), "not bracket-quoted: {result}");
        assert!(
            result.contains("AS NVARCHAR(MAX))"),
            "wrong cast target: {result}"
        );
    }

    /// The whole-row wildcard cast only exists on Postgres; other backends get
    /// a local error instead of SQL that the database is guaranteed to reject.
    #[test]
    fn test_apply_filter_wildcard_errors_off_postgres() {
        let sql = "SELECT * FROM users";
        assert!(apply_filter(sql, "x", None, DbBackend::Mysql).is_err());
        assert!(apply_filter(sql, "x", None, DbBackend::SqlServer).is_err());
        assert!(apply_filter(sql, "x", None, DbBackend::Sqlite).is_err());
        assert!(apply_filter(sql, "x", None, DbBackend::Postgres).is_ok());
    }

    /// A mixed-case sort column must be quoted, or Postgres folds it to
    /// lowercase and sorts by a column that may not exist.
    #[test]
    fn test_apply_order_quotes_identifier() {
        let sql = "SELECT * FROM t";
        let pg = apply_order(
            sql,
            "createdAt",
            SortDirection::Ascending,
            DbBackend::Postgres,
        )
        .unwrap();
        assert!(pg.contains("\"createdAt\""), "{pg}");
        let my = apply_order(sql, "createdAt", SortDirection::Ascending, DbBackend::Mysql).unwrap();
        assert!(my.contains("`createdAt`"), "{my}");
    }

    /// SQL Server rejects ORDER BY in a subquery without OFFSET/TOP, and the
    /// paginator wraps this SQL as one — so the ORDER BY must carry OFFSET.
    #[test]
    fn test_apply_order_sqlserver_adds_offset() {
        let result = apply_order(
            "SELECT * FROM t",
            "name",
            SortDirection::Ascending,
            DbBackend::SqlServer,
        )
        .unwrap();
        assert!(result.to_uppercase().contains("ORDER BY"), "{result}");
        assert!(result.to_uppercase().contains("OFFSET 0 ROWS"), "{result}");
    }

    /// A typed `%` is a literal to match, not a wildcard: it must be escaped
    /// and the clause must declare its ESCAPE character.
    #[test]
    fn test_apply_filter_escapes_like_wildcards() {
        let result =
            apply_filter("SELECT * FROM t", "name:50%", None, DbBackend::Postgres).unwrap();
        assert!(result.contains("50\\%"), "wildcard not escaped: {result}");
        assert!(result.contains("ESCAPE '\\'"), "no ESCAPE clause: {result}");
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
