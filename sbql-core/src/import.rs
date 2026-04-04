//! Data import from CSV and JSON files into database tables.
//!
//! Rows are read in batches and inserted via multi-row INSERT statements
//! with inline-escaped string values. Each backend uses its own identifier
//! quoting style; parameter binding is intentionally avoided so the same
//! logic works across PG, MySQL, and SQLite without sqlx generic-executor
//! gymnastics.

use std::fs::File;
use std::io::BufReader;

use sqlx::{MySqlPool, PgPool, SqlitePool};

use crate::error::{Result, SbqlError};
use crate::pool::DbPool;

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
fn escape_value(s: &str) -> String {
    if s.is_empty() {
        "NULL".to_owned()
    } else {
        format!("'{}'", s.replace('\'', "''"))
    }
}

/// Quote an identifier with double-quotes (PG, SQLite).
fn quote_double(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Quote an identifier with backticks (MySQL).
fn quote_backtick(name: &str) -> String {
    format!("`{}`", name.replace('`', "``"))
}

// ---------------------------------------------------------------------------
// Row parsing helpers
// ---------------------------------------------------------------------------

/// Read CSV headers and records into column names and row batches.
fn read_csv(path: &str) -> Result<(Vec<String>, Vec<Vec<String>>)> {
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

    let mut rows = Vec::new();
    for result in rdr.records() {
        let record = result.map_err(|e| SbqlError::Import(e.to_string()))?;
        rows.push(record.iter().map(|s| s.to_string()).collect());
    }
    Ok((headers, rows))
}

/// Read a JSON file and extract column names and rows.
///
/// Expects either a top-level JSON array of objects, or a top-level object
/// containing a single key that maps to an array of objects.
fn read_json(path: &str) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let file = File::open(path).map_err(|e| SbqlError::Import(e.to_string()))?;
    let reader = BufReader::new(file);
    let data: serde_json::Value =
        serde_json::from_reader(reader).map_err(|e| SbqlError::Import(e.to_string()))?;

    let items: &[serde_json::Value] = match &data {
        serde_json::Value::Array(arr) => arr.as_slice(),
        serde_json::Value::Object(map) => {
            // Find the first key whose value is an array of objects.
            let mut found: Option<&[serde_json::Value]> = None;
            for (_key, val) in map {
                if let serde_json::Value::Array(arr) = val {
                    if arr.first().map_or(false, |v| v.is_object()) {
                        found = Some(arr.as_slice());
                        break;
                    }
                }
            }
            found.ok_or_else(|| {
                SbqlError::Import("Expected JSON array or object containing an array".into())
            })?
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

    let mut rows = Vec::with_capacity(items.len());
    for item in items {
        if let serde_json::Value::Object(map) = item {
            let row: Vec<String> = columns
                .iter()
                .map(|col| match map.get(col) {
                    Some(serde_json::Value::String(s)) => s.clone(),
                    Some(serde_json::Value::Number(n)) => n.to_string(),
                    Some(serde_json::Value::Bool(b)) => b.to_string(),
                    Some(serde_json::Value::Null) | None => String::new(),
                    Some(v) => v.to_string(),
                })
                .collect();
            rows.push(row);
        }
    }

    Ok((columns, rows))
}

// ---------------------------------------------------------------------------
// Flush helpers — one per backend
// ---------------------------------------------------------------------------

fn build_values_clause(batch: &[Vec<String>]) -> String {
    batch
        .iter()
        .map(|row| {
            let vals = row.iter().map(|v| escape_value(v)).collect::<Vec<_>>().join(", ");
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
    let col_list = columns.iter().map(|c| quote_double(c)).collect::<Vec<_>>().join(", ");
    let table_ref = format!("{}.{}", quote_double(schema), quote_double(table));
    let sql = format!(
        "INSERT INTO {} ({}) VALUES {}",
        table_ref,
        col_list,
        build_values_clause(batch)
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
    let col_list = columns.iter().map(|c| quote_double(c)).collect::<Vec<_>>().join(", ");
    let sql = format!(
        "INSERT INTO {} ({}) VALUES {}",
        quote_double(table),
        col_list,
        build_values_clause(batch)
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
    let col_list = columns.iter().map(|c| quote_backtick(c)).collect::<Vec<_>>().join(", ");
    let table_ref = format!("{}.{}", quote_backtick(schema), quote_backtick(table));
    let sql = format!(
        "INSERT INTO {} ({}) VALUES {}",
        table_ref,
        col_list,
        build_values_clause(batch)
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
    let (headers, rows) = read_csv(path)?;
    import_rows_pg(pool, schema, table, &headers, rows).await
}

async fn import_csv_sqlite(pool: &SqlitePool, path: &str, table: &str) -> Result<u64> {
    let (headers, rows) = read_csv(path)?;
    import_rows_sqlite(pool, table, &headers, rows).await
}

async fn import_csv_mysql(
    pool: &MySqlPool,
    path: &str,
    schema: &str,
    table: &str,
) -> Result<u64> {
    let (headers, rows) = read_csv(path)?;
    import_rows_mysql(pool, schema, table, &headers, rows).await
}

// ---------------------------------------------------------------------------
// Per-backend JSON import
// ---------------------------------------------------------------------------

async fn import_json_pg(pool: &PgPool, path: &str, schema: &str, table: &str) -> Result<u64> {
    let (columns, rows) = read_json(path)?;
    if columns.is_empty() {
        return Ok(0);
    }
    import_rows_pg(pool, schema, table, &columns, rows).await
}

async fn import_json_sqlite(pool: &SqlitePool, path: &str, table: &str) -> Result<u64> {
    let (columns, rows) = read_json(path)?;
    if columns.is_empty() {
        return Ok(0);
    }
    import_rows_sqlite(pool, table, &columns, rows).await
}

async fn import_json_mysql(
    pool: &MySqlPool,
    path: &str,
    schema: &str,
    table: &str,
) -> Result<u64> {
    let (columns, rows) = read_json(path)?;
    if columns.is_empty() {
        return Ok(0);
    }
    import_rows_mysql(pool, schema, table, &columns, rows).await
}

// ---------------------------------------------------------------------------
// Row batching — shared logic per backend
// ---------------------------------------------------------------------------

async fn import_rows_pg(
    pool: &PgPool,
    schema: &str,
    table: &str,
    columns: &[String],
    rows: Vec<Vec<String>>,
) -> Result<u64> {
    let mut count: u64 = 0;
    for batch in rows.chunks(BATCH_SIZE) {
        count += flush_batch_pg(pool, schema, table, columns, batch).await?;
    }
    Ok(count)
}

async fn import_rows_sqlite(
    pool: &SqlitePool,
    table: &str,
    columns: &[String],
    rows: Vec<Vec<String>>,
) -> Result<u64> {
    let mut count: u64 = 0;
    for batch in rows.chunks(BATCH_SIZE) {
        count += flush_batch_sqlite(pool, table, columns, batch).await?;
    }
    Ok(count)
}

async fn import_rows_mysql(
    pool: &MySqlPool,
    schema: &str,
    table: &str,
    columns: &[String],
    rows: Vec<Vec<String>>,
) -> Result<u64> {
    let mut count: u64 = 0;
    for batch in rows.chunks(BATCH_SIZE) {
        count += flush_batch_mysql(pool, schema, table, columns, batch).await?;
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

    #[test]
    fn test_quote_double() {
        assert_eq!(quote_double("col"), "\"col\"");
        assert_eq!(quote_double("col\"x"), "\"col\"\"x\"");
    }

    #[test]
    fn test_quote_backtick() {
        assert_eq!(quote_backtick("col"), "`col`");
        assert_eq!(quote_backtick("col`x"), "`col``x`");
    }

    #[test]
    fn test_build_values_clause() {
        let batch = vec![
            vec!["a".into(), "b".into()],
            vec!["c".into(), "".into()],
        ];
        let clause = build_values_clause(&batch);
        assert_eq!(clause, "('a', 'b'), ('c', NULL)");
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
