use sqlx::postgres::PgRow;
use sqlx::sqlite::SqliteRow;
use sqlx::{Column, Decode, PgPool, Postgres, Row, SqlitePool, TypeInfo, ValueRef};

use crate::error::Result;
use crate::pool::DbPool;

pub const PAGE_SIZE: usize = 100;

/// The result of a paginated query execution.
#[derive(Debug, Clone, Default)]
pub struct QueryResult {
    /// Column names in order.
    pub columns: Vec<String>,
    /// Each row is a `Vec<String>` of stringified cell values.
    /// NULL values are represented as an empty string.
    pub rows: Vec<Vec<String>>,
    /// The zero-based page index that was fetched.
    pub page: usize,
    /// Whether there might be more pages after this one.
    pub has_next_page: bool,
}

/// Execute a raw SQL string and return the first `PAGE_SIZE` rows of page
/// `page` (0-indexed).
pub async fn execute_page(pool: &DbPool, sql: &str, page: usize) -> Result<QueryResult> {
    match pool {
        DbPool::Postgres(pg) => execute_page_pg(pg, sql, page).await,
        DbPool::Sqlite(sq) => execute_page_sqlite(sq, sql, page).await,
        DbPool::Redis(cm) => execute_page_redis(cm, sql).await,
    }
}

/// Suggest distinct values for a column using prefix search.
pub async fn suggest_distinct_values(
    pool: &DbPool,
    sql: &str,
    column: &str,
    prefix: &str,
    limit: usize,
) -> Result<Vec<String>> {
    match pool {
        DbPool::Postgres(pg) => suggest_distinct_values_pg(pg, sql, column, prefix, limit).await,
        DbPool::Sqlite(sq) => suggest_distinct_values_sqlite(sq, sql, column, prefix, limit).await,
        DbPool::Redis(_) => Ok(vec![]),
    }
}

// ---------------------------------------------------------------------------
// PostgreSQL implementation
// ---------------------------------------------------------------------------

async fn execute_page_pg(pool: &PgPool, sql: &str, page: usize) -> Result<QueryResult> {
    let paginated = build_paginated_sql(sql, page);
    let rows: Vec<PgRow> = sqlx::query(&paginated).fetch_all(pool).await?;

    let has_next_page = rows.len() > PAGE_SIZE;
    let rows_to_show = if has_next_page {
        &rows[..PAGE_SIZE]
    } else {
        &rows[..]
    };

    let columns: Vec<String> = rows_to_show
        .first()
        .map(|r| r.columns().iter().map(|c| c.name().to_owned()).collect())
        .unwrap_or_default();

    let result_rows: Vec<Vec<String>> = rows_to_show.iter().map(pg_row_to_strings).collect();

    Ok(QueryResult {
        columns,
        rows: result_rows,
        page,
        has_next_page,
    })
}

async fn suggest_distinct_values_pg(
    pool: &PgPool,
    sql: &str,
    column: &str,
    prefix: &str,
    limit: usize,
) -> Result<Vec<String>> {
    let trimmed = sql.trim_end_matches(';').trim();
    let col_ident = quote_ident(column);
    let stmt = format!(
        "SELECT DISTINCT CAST(_sbql_s.{col_ident} AS TEXT) AS v FROM ({trimmed}) AS _sbql_s WHERE CAST(_sbql_s.{col_ident} AS TEXT) ILIKE $1 ORDER BY v LIMIT $2"
    );
    let pattern = format!("{}%", prefix.replace('%', "\\%").replace('_', "\\_"));
    let rows = sqlx::query(&stmt)
        .bind(pattern)
        .bind(limit as i64)
        .fetch_all(pool)
        .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        if let Ok(Some(v)) = row.try_get::<Option<String>, _>("v") {
            out.push(v);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// SQLite implementation
// ---------------------------------------------------------------------------

async fn execute_page_sqlite(pool: &SqlitePool, sql: &str, page: usize) -> Result<QueryResult> {
    let paginated = build_paginated_sql(sql, page);
    let rows: Vec<SqliteRow> = sqlx::query(&paginated).fetch_all(pool).await?;

    let has_next_page = rows.len() > PAGE_SIZE;
    let rows_to_show = if has_next_page {
        &rows[..PAGE_SIZE]
    } else {
        &rows[..]
    };

    let columns: Vec<String> = rows_to_show
        .first()
        .map(|r| r.columns().iter().map(|c| c.name().to_owned()).collect())
        .unwrap_or_default();

    let result_rows: Vec<Vec<String>> = rows_to_show.iter().map(sqlite_row_to_strings).collect();

    Ok(QueryResult {
        columns,
        rows: result_rows,
        page,
        has_next_page,
    })
}

async fn suggest_distinct_values_sqlite(
    pool: &SqlitePool,
    sql: &str,
    column: &str,
    prefix: &str,
    limit: usize,
) -> Result<Vec<String>> {
    let trimmed = sql.trim_end_matches(';').trim();
    let col_ident = quote_ident(column);
    // SQLite: use LIKE with COLLATE NOCASE instead of ILIKE
    let stmt = format!(
        "SELECT DISTINCT CAST(_sbql_s.{col_ident} AS TEXT) AS v FROM ({trimmed}) AS _sbql_s WHERE CAST(_sbql_s.{col_ident} AS TEXT) LIKE $1 COLLATE NOCASE ORDER BY v LIMIT $2"
    );
    let pattern = format!("{}%", prefix.replace('%', "\\%").replace('_', "\\_"));
    let rows = sqlx::query(&stmt)
        .bind(pattern)
        .bind(limit as i64)
        .fetch_all(pool)
        .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        if let Ok(Some(v)) = row.try_get::<Option<String>, _>("v") {
            out.push(v);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Redis implementation
// ---------------------------------------------------------------------------

async fn execute_page_redis(
    cm: &redis::aio::ConnectionManager,
    command_str: &str,
) -> Result<QueryResult> {
    let tokens = tokenize_redis_command(command_str);
    if tokens.is_empty() {
        return Ok(QueryResult::default());
    }

    let mut cmd = redis::cmd(&tokens[0]);
    for arg in &tokens[1..] {
        cmd.arg(arg.as_str());
    }

    let value: redis::Value = cmd.query_async(&mut cm.clone()).await?;
    Ok(redis_value_to_query_result(&value))
}

/// Tokenize a Redis command string, respecting double-quoted and single-quoted strings.
fn tokenize_redis_command(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }
        if ch == '"' || ch == '\'' {
            let quote = ch;
            chars.next(); // consume opening quote
            let mut token = String::new();
            while let Some(&c) = chars.peek() {
                if c == quote {
                    chars.next(); // consume closing quote
                    break;
                }
                if c == '\\' {
                    chars.next();
                    if let Some(&escaped) = chars.peek() {
                        token.push(escaped);
                        chars.next();
                    }
                } else {
                    token.push(c);
                    chars.next();
                }
            }
            tokens.push(token);
        } else {
            let mut token = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                token.push(c);
                chars.next();
            }
            tokens.push(token);
        }
    }

    tokens
}

/// Build a single-value `QueryResult` (one column "value", one row).
fn single_value_result(val: String) -> QueryResult {
    QueryResult {
        columns: vec!["value".into()],
        rows: vec![vec![val]],
        page: 0,
        has_next_page: false,
    }
}

/// Build a key-value `QueryResult` from pre-built rows.
fn kv_result(col_a: &str, col_b: &str, rows: Vec<Vec<String>>) -> QueryResult {
    QueryResult {
        columns: vec![col_a.into(), col_b.into()],
        rows,
        page: 0,
        has_next_page: false,
    }
}

/// Convert a `redis::Value` into a `QueryResult` for display.
fn redis_value_to_query_result(value: &redis::Value) -> QueryResult {
    match value {
        redis::Value::Nil => single_value_result("(nil)".into()),
        redis::Value::Int(i) => single_value_result(i.to_string()),
        redis::Value::BulkString(b) => single_value_result(String::from_utf8_lossy(b).into_owned()),
        redis::Value::SimpleString(s) => single_value_result(s.clone()),
        redis::Value::Okay => single_value_result("OK".into()),
        redis::Value::Array(arr) => {
            // Check if this looks like HGETALL output (even-length, key-value pairs)
            if arr.len() >= 2 && arr.len() % 2 == 0 && arr.iter().all(is_string_like) {
                let rows = arr
                    .chunks(2)
                    .map(|pair| {
                        vec![
                            redis_value_to_string(&pair[0]),
                            redis_value_to_string(&pair[1]),
                        ]
                    })
                    .collect();
                kv_result("field", "value", rows)
            } else {
                let rows = arr
                    .iter()
                    .enumerate()
                    .map(|(i, v)| vec![i.to_string(), redis_value_to_string(v)])
                    .collect();
                kv_result("index", "value", rows)
            }
        }
        redis::Value::Double(f) => single_value_result(f.to_string()),
        redis::Value::Boolean(b) => single_value_result(b.to_string()),
        redis::Value::VerbatimString { text, .. } => QueryResult {
            columns: vec!["value".into()],
            rows: text.lines().map(|l| vec![l.to_string()]).collect(),
            page: 0,
            has_next_page: false,
        },
        redis::Value::BigNumber(n) => single_value_result(n.to_string()),
        redis::Value::Map(pairs) => {
            let rows = pairs
                .iter()
                .map(|(k, v)| vec![redis_value_to_string(k), redis_value_to_string(v)])
                .collect();
            kv_result("field", "value", rows)
        }
        redis::Value::Set(items) => {
            let rows = items
                .iter()
                .enumerate()
                .map(|(i, v)| vec![i.to_string(), redis_value_to_string(v)])
                .collect();
            kv_result("index", "value", rows)
        }
        redis::Value::Attribute { data, .. } => redis_value_to_query_result(data),
        redis::Value::Push { data, .. } => {
            let rows = data
                .iter()
                .enumerate()
                .map(|(i, v)| vec![i.to_string(), redis_value_to_string(v)])
                .collect();
            kv_result("index", "value", rows)
        }
        redis::Value::ServerError(e) => QueryResult {
            columns: vec!["error".into()],
            rows: vec![vec![format!("ERR {}", e.details().unwrap_or_default())]],
            page: 0,
            has_next_page: false,
        },
    }
}

fn redis_value_to_string(value: &redis::Value) -> String {
    match value {
        redis::Value::Nil => "(nil)".into(),
        redis::Value::Int(i) => i.to_string(),
        redis::Value::BulkString(b) => String::from_utf8_lossy(b).into_owned(),
        redis::Value::SimpleString(s) => s.clone(),
        redis::Value::Okay => "OK".into(),
        redis::Value::Double(f) => f.to_string(),
        redis::Value::Boolean(b) => b.to_string(),
        redis::Value::BigNumber(n) => n.to_string(),
        redis::Value::Array(arr) => format!(
            "[{}]",
            arr.iter()
                .map(redis_value_to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        redis::Value::VerbatimString { text, .. } => text.clone(),
        redis::Value::Map(pairs) => format!(
            "{{{}}}",
            pairs
                .iter()
                .map(|(k, v)| format!("{}: {}", redis_value_to_string(k), redis_value_to_string(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        redis::Value::Set(items) => format!(
            "{{{}}}",
            items
                .iter()
                .map(redis_value_to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        redis::Value::ServerError(e) => format!("ERR {}", e.details().unwrap_or_default()),
        redis::Value::Attribute { data, .. } => redis_value_to_string(data),
        redis::Value::Push { data, .. } => format!(
            "[{}]",
            data.iter()
                .map(redis_value_to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn is_string_like(value: &redis::Value) -> bool {
    matches!(
        value,
        redis::Value::BulkString(_) | redis::Value::SimpleString(_) | redis::Value::Int(_)
    )
}

// ---------------------------------------------------------------------------
// Row conversion helpers
// ---------------------------------------------------------------------------

/// Convert a single `PgRow` into a `Vec<String>`, stringifying every column.
fn pg_row_to_strings(row: &PgRow) -> Vec<String> {
    row.columns()
        .iter()
        .map(|col| {
            let idx = col.ordinal();
            let type_name = col.type_info().name();
            pg_value_to_string(row, idx, type_name)
        })
        .collect()
}

/// Convert a single `SqliteRow` into a `Vec<String>`.
fn sqlite_row_to_strings(row: &SqliteRow) -> Vec<String> {
    row.columns()
        .iter()
        .map(|col| {
            let idx = col.ordinal();
            sqlite_value_to_string(row, idx)
        })
        .collect()
}

/// Stringify a SQLite column value.
fn sqlite_value_to_string(row: &SqliteRow, idx: usize) -> String {
    // Check for NULL first
    if let Ok(raw) = row.try_get_raw(idx) {
        if raw.is_null() {
            return String::new();
        }
    }

    // Try String first (covers TEXT)
    if let Ok(v) = row.try_get::<String, _>(idx) {
        return v;
    }
    // Try i64 (covers INTEGER)
    if let Ok(v) = row.try_get::<i64, _>(idx) {
        return v.to_string();
    }
    // Try f64 (covers REAL)
    if let Ok(v) = row.try_get::<f64, _>(idx) {
        return v.to_string();
    }
    // Try bool
    if let Ok(v) = row.try_get::<bool, _>(idx) {
        return v.to_string();
    }
    // Try Vec<u8> (covers BLOB)
    if let Ok(v) = row.try_get::<Vec<u8>, _>(idx) {
        return format!("\\x{}", hex_encode(&v));
    }

    "<unknown>".to_string()
}

/// Stringify a PostgreSQL column value by its type name.
/// Falls back to a UTF-8 byte decode for unknown types.
fn pg_value_to_string(row: &PgRow, idx: usize, type_name: &str) -> String {
    // Try the most common types first.
    macro_rules! try_get {
        ($t:ty) => {{
            if let Ok(v) = row.try_get::<Option<$t>, _>(idx) {
                return match v {
                    Some(val) => val.to_string(),
                    None => String::new(),
                };
            }
        }};
    }

    let upper = type_name.to_uppercase();
    let upper = upper.as_str();

    // --- Booleans ---
    if matches!(upper, "BOOL" | "BOOLEAN") {
        try_get!(bool);
    }

    // --- Integers ---
    if matches!(upper, "INT2" | "SMALLINT" | "SMALLSERIAL") {
        try_get!(i16);
    }
    if matches!(upper, "INT4" | "INT" | "INTEGER" | "SERIAL") {
        try_get!(i32);
    }
    if matches!(upper, "INT8" | "BIGINT" | "BIGSERIAL") {
        try_get!(i64);
    }

    // --- Floats ---
    if matches!(upper, "FLOAT4" | "REAL") {
        try_get!(f32);
    }
    if matches!(upper, "FLOAT8" | "DOUBLE PRECISION") {
        try_get!(f64);
    }

    // --- Exact numeric ---
    if matches!(upper, "NUMERIC" | "DECIMAL") {
        if let Ok(v) = row.try_get::<Option<sqlx::types::BigDecimal>, _>(idx) {
            return match v {
                Some(val) => val.to_string(),
                None => String::new(),
            };
        }
    }

    // --- OID and other unsigned ints (sqlx maps OID to i64 on Postgres) ---
    if matches!(
        upper,
        "OID"
            | "REGPROC"
            | "REGPROCEDURE"
            | "REGOPER"
            | "REGOPERATOR"
            | "REGCLASS"
            | "REGTYPE"
            | "REGCONFIG"
            | "REGDICTIONARY"
    ) {
        try_get!(i64);
    }

    // --- Text-like (String covers MONEY, CIDR, INET,
    //     MACADDR, BIT, VARBIT, XML, TSVECTOR, TSQUERY, PATH, POINT,
    //     LINE, LSEG, BOX, POLYGON, CIRCLE, PG_LSN and anything unknown) ---
    if matches!(
        upper,
        "TEXT"
            | "VARCHAR"
            | "CHAR"
            | "BPCHAR"
            | "NAME"
            | "CITEXT"
            | "MONEY"
            | "INET"
            | "CIDR"
            | "MACADDR"
            | "MACADDR8"
            | "BIT"
            | "VARBIT"
            | "XML"
            | "TSVECTOR"
            | "TSQUERY"
            | "POINT"
            | "LINE"
            | "LSEG"
            | "BOX"
            | "PATH"
            | "POLYGON"
            | "CIRCLE"
            | "PG_LSN"
            | "TXID_SNAPSHOT"
            | "INTERVAL"
            | "INT4RANGE"
            | "INT8RANGE"
            | "NUMRANGE"
            | "TSRANGE"
            | "TSTZRANGE"
            | "DATERANGE"
    ) {
        try_get!(String);
    }

    // --- UUID ---
    if upper == "UUID" {
        try_get!(uuid::Uuid);
    }

    // --- Date / time ---
    if upper == "TIMESTAMPTZ" {
        if let Ok(v) = row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(idx) {
            return match v {
                Some(val) => val.to_rfc3339(),
                None => String::new(),
            };
        }
    }
    if upper == "TIMESTAMP" {
        if let Ok(v) = row.try_get::<Option<chrono::NaiveDateTime>, _>(idx) {
            return match v {
                Some(val) => val.to_string(),
                None => String::new(),
            };
        }
    }
    if upper == "DATE" {
        if let Ok(v) = row.try_get::<Option<chrono::NaiveDate>, _>(idx) {
            return match v {
                Some(val) => val.to_string(),
                None => String::new(),
            };
        }
    }
    if matches!(upper, "TIME" | "TIMETZ") {
        if let Ok(v) = row.try_get::<Option<chrono::NaiveTime>, _>(idx) {
            return match v {
                Some(val) => val.to_string(),
                None => String::new(),
            };
        }
    }

    // --- JSON / JSONB — decode as raw text so we display the JSON string ---
    if matches!(upper, "JSON" | "JSONB") {
        if let Ok(v) = row.try_get::<Option<serde_json::Value>, _>(idx) {
            return match v {
                Some(val) => val.to_string(),
                None => String::new(),
            };
        }
    }

    // --- BYTEA ---
    if upper == "BYTEA" {
        if let Ok(v) = row.try_get::<Option<Vec<u8>>, _>(idx) {
            return match v {
                Some(b) => format!("\\x{}", hex_encode(&b)),
                None => String::new(),
            };
        }
    }

    // --- Array types: try decoding as Vec of the base type, join with commas ---
    if upper.ends_with("[]") || upper.starts_with('_') {
        // Try common array element types
        if let Ok(v) = row.try_get::<Option<Vec<String>>, _>(idx) {
            return match v {
                Some(arr) => format!("{{{}}}", arr.join(",")),
                None => String::new(),
            };
        }
        if let Ok(v) = row.try_get::<Option<Vec<i64>>, _>(idx) {
            return match v {
                Some(arr) => format!(
                    "{{{}}}",
                    arr.iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                None => String::new(),
            };
        }
        if let Ok(v) = row.try_get::<Option<Vec<f64>>, _>(idx) {
            return match v {
                Some(arr) => format!(
                    "{{{}}}",
                    arr.iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                None => String::new(),
            };
        }
        if let Ok(v) = row.try_get::<Option<Vec<bool>>, _>(idx) {
            return match v {
                Some(arr) => format!(
                    "{{{}}}",
                    arr.iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                None => String::new(),
            };
        }
    }

    // --- Universal fallback 1: try as plain String (covers NUMERIC, INTERVAL,
    //     range types, enums, domains and anything the text protocol can encode) ---
    if let Ok(v) = row.try_get::<Option<String>, _>(idx) {
        return v.unwrap_or_default();
    }

    // --- Universal fallback 2: raw bytes → UTF-8 lossy ---
    if let Ok(v) = row.try_get::<Option<Vec<u8>>, _>(idx) {
        return v
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default();
    }

    // --- Universal fallback 3: decode raw value as text ---
    // This catches custom PostgreSQL enums/domains where sqlx dynamic typed
    // decoding may fail through try_get::<String>(), but text decode still works.
    if let Ok(raw) = row.try_get_raw(idx) {
        if raw.is_null() {
            return String::new();
        }
        if let Ok(v) = <String as Decode<Postgres>>::decode(raw) {
            return v;
        }
    }

    // --- Last resort: show type name so it's debuggable ---
    format!("<{}>", type_name)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Append LIMIT/OFFSET to `sql` when there is no existing top-level LIMIT.
pub fn build_paginated_sql(sql: &str, page: usize) -> String {
    let trimmed = sql.trim_end_matches(';').trim();
    let upper = trimmed.to_uppercase();

    let has_limit = upper.contains("LIMIT");

    if has_limit {
        trimmed.to_owned()
    } else {
        let offset = page * PAGE_SIZE;
        format!(
            "SELECT * FROM ({trimmed}) AS _sbql_page LIMIT {} OFFSET {offset}",
            PAGE_SIZE + 1
        )
    }
}

/// Encode a byte slice as lowercase hex.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- build_paginated_sql --

    #[test]
    fn paginated_no_limit_page_0() {
        let result = build_paginated_sql("SELECT * FROM users", 0);
        assert!(result.contains("LIMIT 101"));
        assert!(result.contains("OFFSET 0"));
    }

    #[test]
    fn paginated_no_limit_page_2() {
        let result = build_paginated_sql("SELECT * FROM users", 2);
        assert!(result.contains("LIMIT 101"));
        assert!(result.contains("OFFSET 200"));
    }

    #[test]
    fn paginated_with_existing_limit() {
        let result = build_paginated_sql("SELECT * FROM users LIMIT 10", 0);
        assert_eq!(result, "SELECT * FROM users LIMIT 10");
    }

    #[test]
    fn paginated_strips_semicolon() {
        let result = build_paginated_sql("SELECT * FROM users;", 0);
        assert!(!result.ends_with(';'));
        assert!(result.contains("LIMIT 101"));
    }

    #[test]
    fn paginated_preserves_case() {
        let result = build_paginated_sql("select * from Users WHERE active = true", 0);
        assert!(result.contains("LIMIT 101"));
    }

    #[test]
    fn paginated_existing_limit_case_insensitive() {
        let result = build_paginated_sql("select * from users limit 5", 0);
        assert_eq!(result, "select * from users limit 5");
    }

    // -- hex_encode --

    #[test]
    fn hex_encode_empty() {
        assert_eq!(hex_encode(&[]), "");
    }

    #[test]
    fn hex_encode_bytes() {
        assert_eq!(hex_encode(&[0xDE, 0xAD, 0xBE, 0xEF]), "deadbeef");
    }

    #[test]
    fn hex_encode_zeros() {
        assert_eq!(hex_encode(&[0x00, 0x01, 0x0F]), "00010f");
    }

    // -- quote_ident --

    #[test]
    fn quote_ident_simple() {
        assert_eq!(quote_ident("column_name"), "\"column_name\"");
    }

    #[test]
    fn quote_ident_with_quotes() {
        assert_eq!(quote_ident("col\"name"), "\"col\"\"name\"");
    }
}
