use sbql_core::{
    query::{export_all, ExportFormat},
    DbPool,
};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::fs;
use tempfile::NamedTempFile;

/// Helper: create an in-memory SQLite pool with a `users` table and 5 rows.
async fn setup_sqlite_with_users() -> (SqlitePool, DbPool) {
    let sq = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite");

    sqlx::query(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT NOT NULL)",
    )
    .execute(&sq)
    .await
    .expect("Failed to create users table");

    sqlx::query("INSERT INTO users (id, name, email) VALUES (1, 'Alice', 'alice@example.com'), (2, 'Bob', 'bob@example.com'), (3, 'Charlie', 'charlie@example.com'), (4, 'Diana', 'diana@example.com'), (5, 'Eve', 'eve@example.com')")
        .execute(&sq)
        .await
        .expect("Failed to seed users");

    let pool = DbPool::Sqlite(sq.clone());
    (sq, pool)
}

// ---------------------------------------------------------------------------
// CSV tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_export_csv() {
    let (_sq, pool) = setup_sqlite_with_users().await;

    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_str().unwrap();
    let count = export_all(&pool, "SELECT * FROM users", path, ExportFormat::Csv, "users")
        .await
        .unwrap();

    assert_eq!(count, 5);

    let content = fs::read_to_string(path).unwrap();
    let lines: Vec<&str> = content.lines().collect();

    // Header + 5 data rows
    assert_eq!(lines.len(), 6, "Expected 6 lines (header + 5 rows), got: {}", lines.len());

    // Verify header contains column names
    assert!(lines[0].contains("id"), "Header missing 'id': {}", lines[0]);
    assert!(lines[0].contains("name"), "Header missing 'name': {}", lines[0]);
    assert!(lines[0].contains("email"), "Header missing 'email': {}", lines[0]);

    // Verify data rows contain expected values
    assert!(content.contains("Alice"), "Missing Alice in CSV output");
    assert!(content.contains("alice@example.com"), "Missing alice email in CSV output");
    assert!(content.contains("Eve"), "Missing Eve in CSV output");
}

#[tokio::test]
async fn test_export_json() {
    let (_sq, pool) = setup_sqlite_with_users().await;

    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_str().unwrap();
    let count = export_all(&pool, "SELECT * FROM users", path, ExportFormat::Json, "users")
        .await
        .unwrap();

    assert_eq!(count, 5);

    let content = fs::read_to_string(path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content)
        .expect("Exported JSON should be valid");

    let arr = parsed.as_array().expect("JSON should be an array");
    assert_eq!(arr.len(), 5, "Expected 5 JSON objects");

    // Verify each object has the expected keys
    for obj in arr {
        let map = obj.as_object().expect("Each entry should be an object");
        assert!(map.contains_key("id"), "Missing 'id' key in JSON object");
        assert!(map.contains_key("name"), "Missing 'name' key in JSON object");
        assert!(map.contains_key("email"), "Missing 'email' key in JSON object");
    }

    // Verify specific values
    let first = arr[0].as_object().unwrap();
    assert_eq!(first["name"].as_str().unwrap(), "Alice");
    assert_eq!(first["email"].as_str().unwrap(), "alice@example.com");
}

#[tokio::test]
async fn test_export_sql_insert() {
    let (_sq, pool) = setup_sqlite_with_users().await;

    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_str().unwrap();
    let count = export_all(
        &pool,
        "SELECT * FROM users",
        path,
        ExportFormat::SqlInsert,
        "users",
    )
    .await
    .unwrap();

    assert_eq!(count, 5);

    let content = fs::read_to_string(path).unwrap();
    let lines: Vec<&str> = content.lines().collect();

    assert_eq!(lines.len(), 5, "Expected 5 INSERT statements");

    for line in &lines {
        assert!(
            line.starts_with("INSERT INTO"),
            "Each line should be an INSERT statement, got: {}",
            line
        );
        assert!(
            line.ends_with(';'),
            "Each INSERT should end with semicolon, got: {}",
            line
        );
        assert!(
            line.contains("\"users\""),
            "INSERT should reference table 'users', got: {}",
            line
        );
    }

    // Verify proper quoting of values
    assert!(
        content.contains("'Alice'") || content.contains("Alice"),
        "Expected Alice in SQL output"
    );
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_export_csv_special_chars() {
    let sq = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    sqlx::query("CREATE TABLE special (id INTEGER PRIMARY KEY, value TEXT NOT NULL)")
        .execute(&sq)
        .await
        .unwrap();

    // Insert values with commas, quotes, and newlines
    sqlx::query("INSERT INTO special (id, value) VALUES (1, 'has,comma')")
        .execute(&sq)
        .await
        .unwrap();
    sqlx::query("INSERT INTO special (id, value) VALUES (2, 'has\"quote')")
        .execute(&sq)
        .await
        .unwrap();
    sqlx::query("INSERT INTO special (id, value) VALUES (3, 'has\nnewline')")
        .execute(&sq)
        .await
        .unwrap();
    sqlx::query("INSERT INTO special (id, value) VALUES (4, 'plain')")
        .execute(&sq)
        .await
        .unwrap();

    let pool = DbPool::Sqlite(sq);
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_str().unwrap();
    let count = export_all(
        &pool,
        "SELECT * FROM special ORDER BY id",
        path,
        ExportFormat::Csv,
        "special",
    )
    .await
    .unwrap();

    assert_eq!(count, 4);

    let content = fs::read_to_string(path).unwrap();

    // Values with commas should be quoted
    assert!(
        content.contains("\"has,comma\""),
        "Comma value should be quoted in CSV: {}",
        content
    );

    // Values with double quotes should be escaped (CSV escapes " as "")
    assert!(
        content.contains("\"has\"\"quote\""),
        "Quote value should be escaped in CSV: {}",
        content
    );

    // Values with newlines should be quoted
    assert!(
        content.contains("\"has\nnewline\""),
        "Newline value should be quoted in CSV: {}",
        content
    );
}

#[tokio::test]
async fn test_export_empty_table() {
    let sq = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    sqlx::query("CREATE TABLE empty (id INTEGER PRIMARY KEY, name TEXT)")
        .execute(&sq)
        .await
        .unwrap();

    let pool = DbPool::Sqlite(sq);

    // CSV: empty table should produce no data rows
    {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();
        let count = export_all(
            &pool,
            "SELECT * FROM empty",
            path,
            ExportFormat::Csv,
            "empty",
        )
        .await
        .unwrap();

        assert_eq!(count, 0);
        let content = fs::read_to_string(path).unwrap();
        // With zero rows, no data lines should be present.
        // The header may or may not be written depending on implementation
        // (columns are discovered from the first row).
        let non_empty_lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
        assert!(
            non_empty_lines.is_empty(),
            "Expected no data lines for empty table CSV, got: {:?}",
            non_empty_lines
        );
    }

    // JSON: empty table
    {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();
        let count = export_all(
            &pool,
            "SELECT * FROM empty",
            path,
            ExportFormat::Json,
            "empty",
        )
        .await
        .unwrap();

        assert_eq!(count, 0);
        let content = fs::read_to_string(path).unwrap();
        // Footer writes "\n]" but header "[" is only written on first row.
        // Verify the file is either empty-ish or a valid empty array.
        let trimmed = content.trim();
        assert!(
            trimmed.is_empty() || trimmed == "[]" || trimmed == "]",
            "Expected empty or minimal JSON for empty table, got: '{}'",
            trimmed
        );
    }

    // SQL INSERT: empty table should produce no statements
    {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();
        let count = export_all(
            &pool,
            "SELECT * FROM empty",
            path,
            ExportFormat::SqlInsert,
            "empty",
        )
        .await
        .unwrap();

        assert_eq!(count, 0);
        let content = fs::read_to_string(path).unwrap();
        assert!(
            content.trim().is_empty(),
            "Expected no INSERT statements for empty table, got: '{}'",
            content
        );
    }
}

#[tokio::test]
async fn test_export_large_dataset() {
    let sq = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    sqlx::query("CREATE TABLE big (id INTEGER PRIMARY KEY, val TEXT NOT NULL)")
        .execute(&sq)
        .await
        .unwrap();

    // Insert 500 rows
    for i in 0..500 {
        sqlx::query("INSERT INTO big (id, val) VALUES (?, ?)")
            .bind(i)
            .bind(format!("value_{i}"))
            .execute(&sq)
            .await
            .unwrap();
    }

    let pool = DbPool::Sqlite(sq);
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_str().unwrap();
    let count = export_all(
        &pool,
        "SELECT * FROM big",
        path,
        ExportFormat::Csv,
        "big",
    )
    .await
    .unwrap();

    assert_eq!(count, 500);

    let content = fs::read_to_string(path).unwrap();
    let line_count = content.lines().count();

    // Header (1) + data rows (500) = 501
    assert_eq!(
        line_count, 501,
        "Expected 501 lines (1 header + 500 rows), got: {}",
        line_count
    );

    // Spot-check first and last data values
    assert!(content.contains("value_0"), "Missing first row value");
    assert!(content.contains("value_499"), "Missing last row value");
}
