//! Data import from CSV and JSON files into database tables.
//!
//! Rows are read in batches and inserted via multi-row INSERT statements
//! with inline-escaped string values. Each backend uses its own identifier
//! quoting style; parameter binding is intentionally avoided so the same
//! logic works across PG, MySQL, and SQLite without sqlx generic-executor
//! gymnastics.
//!
//! Rows travel as an iterator, not a `Vec`: a row source feeds [`batches`],
//! and each batch is flushed to the database before the next one is built, so
//! peak memory is one batch of `BATCH_SIZE` rows rather than the whole file.
//! Importing a multi-gigabyte CSV used to exhaust memory before a single row
//! reached the database.

use std::fs::File;
use std::io::BufReader;

use sqlx::{MySqlPool, PgPool, SqlitePool};

use crate::error::{Result, SbqlError};
use crate::pool::DbPool;
use crate::sql_util::{quote_ident, quote_ident_mysql};

/// The file format to import from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFormat {
    Csv,
    Json,
}

/// Maximum rows per INSERT statement.
const BATCH_SIZE: usize = 500;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Import a CSV or JSON file into a database table.
///
/// Returns the total number of rows inserted.
pub async fn import_file(
    pool: &DbPool,
    path: &str,
    format: ImportFormat,
    schema: &str,
    table: &str,
) -> Result<u64> {
    match pool {
        DbPool::Postgres(pg) => match format {
            ImportFormat::Csv => import_csv_pg(pg, path, schema, table).await,
            ImportFormat::Json => import_json_pg(pg, path, schema, table).await,
        },
        DbPool::Sqlite(sq) => match format {
            ImportFormat::Csv => import_csv_sqlite(sq, path, table).await,
            ImportFormat::Json => import_json_sqlite(sq, path, table).await,
        },
        DbPool::Mysql(my) => match format {
            ImportFormat::Csv => import_csv_mysql(my, path, schema, table).await,
            ImportFormat::Json => import_json_mysql(my, path, schema, table).await,
        },
        _ => Err(SbqlError::Import(
            "Import not supported for this backend".into(),
        )),
    }
}

// ---------------------------------------------------------------------------
// SQL escaping helpers
// ---------------------------------------------------------------------------

/// Escape a string value for inline inclusion in an INSERT statement.
/// Empty strings are mapped to NULL.
///
/// Suitable for Postgres and SQLite, where a backslash inside a standard
/// string literal is an ordinary character and only the quote needs doubling.
fn escape_value(s: &str) -> String {
    if s.is_empty() {
        "NULL".to_owned()
    } else {
        format!("'{}'", s.replace('\'', "''"))
    }
}

/// MySQL variant: with the default sql_mode a backslash starts an escape
/// sequence, so a value ending in `\` would swallow the closing quote and a
/// crafted cell could break out of the literal entirely (SQL injection via an
/// imported file). Backslashes are escaped first, then quotes.
fn escape_value_mysql(s: &str) -> String {
    if s.is_empty() {
        "NULL".to_owned()
    } else {
        format!("'{}'", s.replace('\\', "\\\\").replace('\'', "''"))
    }
}

// ---------------------------------------------------------------------------
// Row sources
//
// CSV streams; JSON does not. `serde_json::from_reader` has to parse an entire
// array before it can hand back any element, so a JSON import's memory floor is
// the parsed `Value` tree — genuine streaming would mean a different parser,
// not a rearrangement of this code. What both formats do share is that rows are
// produced lazily, one at a time, and never accumulated into a full
// `Vec<Vec<String>>`.
// ---------------------------------------------------------------------------

/// Open a CSV file and read its header row, leaving the records unread.
///
/// Returns the record iterator rather than a materialised `Vec` so the caller
/// can flush batches as they fill; `csv::Reader` decodes one record at a time.
fn open_csv(path: &str) -> Result<(Vec<String>, csv::StringRecordsIntoIter<BufReader<File>>)> {
    let file = File::open(path).map_err(|e| SbqlError::Import(e.to_string()))?;
    let mut rdr = csv::Reader::from_reader(BufReader::new(file));

    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| SbqlError::Import(e.to_string()))?
        .iter()
        .map(|h| h.to_string())
        .collect();

    if headers.is_empty() {
        return Err(SbqlError::Import("No columns found in CSV".into()));
    }

    Ok((headers, rdr.into_records()))
}

/// Adapt CSV records to rows, decoding each record only as it is pulled.
fn csv_rows(
    records: csv::StringRecordsIntoIter<BufReader<File>>,
) -> impl Iterator<Item = Result<Vec<String>>> {
    records.map(|record| {
        record
            .map(|r| r.iter().map(|s| s.to_string()).collect())
            .map_err(|e| SbqlError::Import(e.to_string()))
    })
}

/// Parse a JSON file, returning the column names and the items to import.
///
/// Expects either a top-level JSON array of objects, or a top-level object
/// containing a single key that maps to an array of objects.
///
/// The items are moved out of the parsed tree so the rest of it can be dropped;
/// they are turned into rows one at a time by [`json_rows`], which is what
/// keeps this to a single copy of the data.
fn read_json(path: &str) -> Result<(Vec<String>, Vec<serde_json::Value>)> {
    let file = File::open(path).map_err(|e| SbqlError::Import(e.to_string()))?;
    let reader = BufReader::new(file);
    let data: serde_json::Value =
        serde_json::from_reader(reader).map_err(|e| SbqlError::Import(e.to_string()))?;

    let items: Vec<serde_json::Value> = match data {
        serde_json::Value::Array(arr) => arr,
        serde_json::Value::Object(mut map) => {
            // Find the first key whose value is an array of objects, then take
            // that array out of the map so it is not cloned.
            let key = map
                .iter()
                .find(|(_key, val)| {
                    val.as_array()
                        .is_some_and(|arr| arr.first().is_some_and(|v| v.is_object()))
                })
                .map(|(key, _val)| key.clone());
            match key.and_then(|key| map.remove(&key)) {
                Some(serde_json::Value::Array(arr)) => arr,
                _ => {
                    return Err(SbqlError::Import(
                        "Expected JSON array or object containing an array".into(),
                    ))
                }
            }
        }
        _ => {
            return Err(SbqlError::Import("Expected JSON array".into()));
        }
    };

    if items.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    // Collect columns from the first object.
    let columns: Vec<String> = match &items[0] {
        serde_json::Value::Object(map) => map.keys().cloned().collect(),
        _ => return Err(SbqlError::Import("Expected array of objects".into())),
    };

    Ok((columns, items))
}

/// Project one JSON object onto `columns`.
///
/// Values are removed from the map rather than cloned: the object is owned here
/// and dropped straight after, so moving the strings out costs nothing.
fn json_row(
    mut map: serde_json::Map<String, serde_json::Value>,
    columns: &[String],
) -> Vec<String> {
    columns
        .iter()
        .map(|col| match map.remove(col.as_str()) {
            Some(serde_json::Value::String(s)) => s,
            Some(serde_json::Value::Number(n)) => n.to_string(),
            Some(serde_json::Value::Bool(b)) => b.to_string(),
            Some(serde_json::Value::Null) | None => String::new(),
            Some(v) => v.to_string(),
        })
        .collect()
}

/// Adapt parsed JSON items to rows, converting each item only as it is pulled.
fn json_rows(
    columns: &[String],
    items: Vec<serde_json::Value>,
) -> impl Iterator<Item = Result<Vec<String>>> + '_ {
    // Entries that are not objects are skipped, not rejected: only the first
    // item has to be one, because it is where the column list comes from.
    items.into_iter().filter_map(|item| match item {
        serde_json::Value::Object(map) => Some(Ok(json_row(map, columns))),
        _ => None,
    })
}

/// Group a row stream into batches of at most [`BATCH_SIZE`] rows.
///
/// This is the piece that bounds memory: the caller flushes each batch before
/// the next is pulled, so only one batch is ever resident. A row error ends the
/// stream, leaving already-flushed batches in the table — the same partial
/// import you get today when a later INSERT fails, since the import is not
/// wrapped in a transaction.
fn batches<I>(mut rows: I) -> impl Iterator<Item = Result<Vec<Vec<String>>>>
where
    I: Iterator<Item = Result<Vec<String>>>,
{
    std::iter::from_fn(move || {
        let mut batch: Vec<Vec<String>> = Vec::with_capacity(BATCH_SIZE);
        while batch.len() < BATCH_SIZE {
            match rows.next() {
                Some(Ok(row)) => batch.push(row),
                Some(Err(e)) => return Some(Err(e)),
                None => break,
            }
        }
        (!batch.is_empty()).then_some(Ok(batch))
    })
}

// ---------------------------------------------------------------------------
// Flush helpers — one per backend
// ---------------------------------------------------------------------------

fn build_values_clause(batch: &[Vec<String>], escape: fn(&str) -> String) -> String {
    batch
        .iter()
        .map(|row| {
            let vals = row.iter().map(|v| escape(v)).collect::<Vec<_>>().join(", ");
            format!("({})", vals)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

async fn flush_batch_pg(
    pool: &PgPool,
    schema: &str,
    table: &str,
    columns: &[String],
    batch: &[Vec<String>],
) -> Result<u64> {
    let col_list = columns
        .iter()
        .map(|c| quote_ident(c))
        .collect::<Vec<_>>()
        .join(", ");
    let table_ref = format!("{}.{}", quote_ident(schema), quote_ident(table));
    let sql = format!(
        "INSERT INTO {} ({}) VALUES {}",
        table_ref,
        col_list,
        build_values_clause(batch, escape_value)
    );
    sqlx::query(&sql)
        .execute(pool)
        .await
        .map_err(|e| SbqlError::Import(e.to_string()))?;
    Ok(batch.len() as u64)
}

async fn flush_batch_sqlite(
    pool: &SqlitePool,
    table: &str,
    columns: &[String],
    batch: &[Vec<String>],
) -> Result<u64> {
    let col_list = columns
        .iter()
        .map(|c| quote_ident(c))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO {} ({}) VALUES {}",
        quote_ident(table),
        col_list,
        build_values_clause(batch, escape_value)
    );
    sqlx::query(&sql)
        .execute(pool)
        .await
        .map_err(|e| SbqlError::Import(e.to_string()))?;
    Ok(batch.len() as u64)
}

async fn flush_batch_mysql(
    pool: &MySqlPool,
    schema: &str,
    table: &str,
    columns: &[String],
    batch: &[Vec<String>],
) -> Result<u64> {
    let col_list = columns
        .iter()
        .map(|c| quote_ident_mysql(c))
        .collect::<Vec<_>>()
        .join(", ");
    let table_ref = format!("{}.{}", quote_ident_mysql(schema), quote_ident_mysql(table));
    let sql = format!(
        "INSERT INTO {} ({}) VALUES {}",
        table_ref,
        col_list,
        build_values_clause(batch, escape_value_mysql)
    );
    sqlx::query(&sql)
        .execute(pool)
        .await
        .map_err(|e| SbqlError::Import(e.to_string()))?;
    Ok(batch.len() as u64)
}

// ---------------------------------------------------------------------------
// Per-backend CSV import
// ---------------------------------------------------------------------------

async fn import_csv_pg(pool: &PgPool, path: &str, schema: &str, table: &str) -> Result<u64> {
    let (headers, records) = open_csv(path)?;
    import_rows_pg(pool, schema, table, &headers, csv_rows(records)).await
}

async fn import_csv_sqlite(pool: &SqlitePool, path: &str, table: &str) -> Result<u64> {
    let (headers, records) = open_csv(path)?;
    import_rows_sqlite(pool, table, &headers, csv_rows(records)).await
}

async fn import_csv_mysql(pool: &MySqlPool, path: &str, schema: &str, table: &str) -> Result<u64> {
    let (headers, records) = open_csv(path)?;
    import_rows_mysql(pool, schema, table, &headers, csv_rows(records)).await
}

// ---------------------------------------------------------------------------
// Per-backend JSON import
// ---------------------------------------------------------------------------

async fn import_json_pg(pool: &PgPool, path: &str, schema: &str, table: &str) -> Result<u64> {
    let (columns, items) = read_json(path)?;
    if columns.is_empty() {
        return Ok(0);
    }
    import_rows_pg(pool, schema, table, &columns, json_rows(&columns, items)).await
}

async fn import_json_sqlite(pool: &SqlitePool, path: &str, table: &str) -> Result<u64> {
    let (columns, items) = read_json(path)?;
    if columns.is_empty() {
        return Ok(0);
    }
    import_rows_sqlite(pool, table, &columns, json_rows(&columns, items)).await
}

async fn import_json_mysql(pool: &MySqlPool, path: &str, schema: &str, table: &str) -> Result<u64> {
    let (columns, items) = read_json(path)?;
    if columns.is_empty() {
        return Ok(0);
    }
    import_rows_mysql(pool, schema, table, &columns, json_rows(&columns, items)).await
}

// ---------------------------------------------------------------------------
// Row batching — one loop per backend, each naming its own flush (and so its
// own escaper). The loop pulls a batch, flushes it, and drops it before the
// next batch is built.
// ---------------------------------------------------------------------------

async fn import_rows_pg<I>(
    pool: &PgPool,
    schema: &str,
    table: &str,
    columns: &[String],
    rows: I,
) -> Result<u64>
where
    I: Iterator<Item = Result<Vec<String>>>,
{
    let mut count: u64 = 0;
    for batch in batches(rows) {
        count += flush_batch_pg(pool, schema, table, columns, &batch?).await?;
    }
    Ok(count)
}

async fn import_rows_sqlite<I>(
    pool: &SqlitePool,
    table: &str,
    columns: &[String],
    rows: I,
) -> Result<u64>
where
    I: Iterator<Item = Result<Vec<String>>>,
{
    let mut count: u64 = 0;
    for batch in batches(rows) {
        count += flush_batch_sqlite(pool, table, columns, &batch?).await?;
    }
    Ok(count)
}

async fn import_rows_mysql<I>(
    pool: &MySqlPool,
    schema: &str,
    table: &str,
    columns: &[String],
    rows: I,
) -> Result<u64>
where
    I: Iterator<Item = Result<Vec<String>>>,
{
    let mut count: u64 = 0;
    for batch in batches(rows) {
        count += flush_batch_mysql(pool, schema, table, columns, &batch?).await?;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_value_empty() {
        assert_eq!(escape_value(""), "NULL");
    }

    #[test]
    fn test_escape_value_simple() {
        assert_eq!(escape_value("hello"), "'hello'");
    }

    #[test]
    fn test_escape_value_with_quotes() {
        assert_eq!(escape_value("it's"), "'it''s'");
    }

    /// A trailing backslash must not be able to swallow the closing quote on
    /// MySQL, and a crafted cell must stay inside its string literal.
    #[test]
    fn test_escape_value_mysql_neutralizes_backslashes() {
        assert_eq!(escape_value_mysql(r"C:\data\"), r"'C:\\data\\'");
        assert_eq!(
            escape_value_mysql(r"x\', (SELECT 1), ('"),
            r"'x\\'', (SELECT 1), ('''"
        );
        assert_eq!(escape_value_mysql("it's"), "'it''s'");
        assert_eq!(escape_value_mysql(""), "NULL");
    }

    #[test]
    fn test_quote_ident() {
        assert_eq!(quote_ident("col"), "\"col\"");
        assert_eq!(quote_ident("col\"x"), "\"col\"\"x\"");
    }

    #[test]
    fn test_quote_ident_mysql() {
        assert_eq!(quote_ident_mysql("col"), "`col`");
        assert_eq!(quote_ident_mysql("col`x"), "`col``x`");
    }

    #[test]
    fn test_build_values_clause() {
        let batch = vec![vec!["a".into(), "b".into()], vec!["c".into(), "".into()]];
        let clause = build_values_clause(&batch, escape_value);
        assert_eq!(clause, "('a', 'b'), ('c', NULL)");
    }

    /// The MySQL flush path must keep passing the MySQL escaper: with the
    /// generic escaper a trailing backslash would swallow the closing quote.
    #[test]
    fn test_build_values_clause_mysql_escaper() {
        let batch = vec![vec![r"C:\data\".into(), "it's".into()]];
        let clause = build_values_clause(&batch, escape_value_mysql);
        assert_eq!(clause, r"('C:\\data\\', 'it''s')");
    }

    fn rows(count: usize) -> impl Iterator<Item = Result<Vec<String>>> {
        (0..count).map(|i| Ok(vec![i.to_string()]))
    }

    /// The whole point of the streaming refactor: rows leave the reader in
    /// groups of at most `BATCH_SIZE`, with the remainder flushed at the end.
    #[test]
    fn test_batches_groups_by_batch_size() {
        let sizes: Vec<usize> = batches(rows(BATCH_SIZE * 2 + 1))
            .map(|b| b.unwrap().len())
            .collect();
        assert_eq!(sizes, vec![BATCH_SIZE, BATCH_SIZE, 1]);
    }

    /// An exact multiple must not emit a trailing empty batch (which would be
    /// an `INSERT ... VALUES` with nothing after it).
    #[test]
    fn test_batches_exact_multiple() {
        let sizes: Vec<usize> = batches(rows(BATCH_SIZE))
            .map(|b| b.unwrap().len())
            .collect();
        assert_eq!(sizes, vec![BATCH_SIZE]);
    }

    #[test]
    fn test_batches_empty_input() {
        assert_eq!(batches(rows(0)).count(), 0);
    }

    #[test]
    fn test_batches_preserves_row_order() {
        let batch = batches(rows(3)).next().unwrap().unwrap();
        assert_eq!(batch, vec![vec!["0"], vec!["1"], vec!["2"]]);
    }

    #[test]
    fn test_batches_yields_row_error() {
        let source = vec![
            Ok(vec!["ok".to_owned()]),
            Err(SbqlError::Import("bad record".into())),
        ];
        let first = batches(source.into_iter()).next().unwrap();
        assert!(matches!(first, Err(SbqlError::Import(ref m)) if m == "bad record"));
    }

    #[test]
    fn test_json_row_value_kinds() {
        let map = match serde_json::json!({
            "s": "text",
            "n": 42,
            "b": true,
            "null": null,
            "nested": {"k": "v"},
        }) {
            serde_json::Value::Object(map) => map,
            _ => unreachable!(),
        };
        let columns: Vec<String> = ["s", "n", "b", "null", "nested", "missing"]
            .iter()
            .map(|c| (*c).to_owned())
            .collect();
        assert_eq!(
            json_row(map, &columns),
            vec!["text", "42", "true", "", r#"{"k":"v"}"#, ""]
        );
    }

    /// Non-object entries are dropped rather than rejected, as the pre-streaming
    /// reader did — only the first item has to be an object.
    #[test]
    fn test_json_rows_skips_non_objects() {
        let items = vec![
            serde_json::json!({"a": "1"}),
            serde_json::json!("not an object"),
            serde_json::json!({"a": "2"}),
        ];
        let columns = vec!["a".to_owned()];
        let converted: Vec<Vec<String>> = json_rows(&columns, items)
            .map(|r| r.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(converted, vec![vec!["1"], vec!["2"]]);
    }

    #[tokio::test]
    async fn test_import_csv_sqlite_roundtrip() {
        use sqlx::SqlitePool;

        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE test (name TEXT, age TEXT)")
            .execute(&pool)
            .await
            .unwrap();

        // Write a temp CSV
        let dir = std::env::temp_dir();
        let csv_path = dir.join("sbql_import_test.csv");
        std::fs::write(&csv_path, "name,age\nAlice,30\nBob,25\n").unwrap();

        let db_pool = DbPool::Sqlite(pool.clone());
        let count = import_file(
            &db_pool,
            csv_path.to_str().unwrap(),
            ImportFormat::Csv,
            "main",
            "test",
        )
        .await
        .unwrap();

        assert_eq!(count, 2);

        // Verify data
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT name, age FROM test ORDER BY name")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "Alice");
        assert_eq!(rows[0].1, "30");

        let _ = std::fs::remove_file(&csv_path);
    }

    #[tokio::test]
    async fn test_import_json_sqlite_roundtrip() {
        use sqlx::SqlitePool;

        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE test (name TEXT, age TEXT)")
            .execute(&pool)
            .await
            .unwrap();

        let dir = std::env::temp_dir();
        let json_path = dir.join("sbql_import_test.json");
        std::fs::write(
            &json_path,
            r#"[{"name":"Alice","age":"30"},{"name":"Bob","age":"25"}]"#,
        )
        .unwrap();

        let db_pool = DbPool::Sqlite(pool.clone());
        let count = import_file(
            &db_pool,
            json_path.to_str().unwrap(),
            ImportFormat::Json,
            "main",
            "test",
        )
        .await
        .unwrap();

        assert_eq!(count, 2);

        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT name, age FROM test ORDER BY name")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "Alice");

        let _ = std::fs::remove_file(&json_path);
    }

    #[tokio::test]
    async fn test_import_json_nested_object() {
        use sqlx::SqlitePool;

        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE items (id TEXT, val TEXT)")
            .execute(&pool)
            .await
            .unwrap();

        let dir = std::env::temp_dir();
        let json_path = dir.join("sbql_import_nested.json");
        std::fs::write(
            &json_path,
            r#"{"data":[{"id":"1","val":"x"},{"id":"2","val":"y"}]}"#,
        )
        .unwrap();

        let db_pool = DbPool::Sqlite(pool.clone());
        let count = import_file(
            &db_pool,
            json_path.to_str().unwrap(),
            ImportFormat::Json,
            "main",
            "items",
        )
        .await
        .unwrap();

        assert_eq!(count, 2);
        let _ = std::fs::remove_file(&json_path);
    }
}
