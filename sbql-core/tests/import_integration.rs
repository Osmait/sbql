use sbql_core::{
    import::{import_file, ImportFormat},
    DbPool,
};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Row, SqlitePool};
use std::io::Write;

/// Helper: create an in-memory SQLite pool with a `users` table (all TEXT columns).
async fn setup_sqlite_with_table() -> (SqlitePool, DbPool) {
    let sq = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite");

    sqlx::query("CREATE TABLE users (name TEXT, age TEXT, email TEXT)")
        .execute(&sq)
        .await
        .expect("Failed to create users table");

    let pool = DbPool::Sqlite(sq.clone());
    (sq, pool)
}

// ---------------------------------------------------------------------------
// CSV tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_import_csv() {
    let (sq, pool) = setup_sqlite_with_table().await;

    let mut file = tempfile::NamedTempFile::new().unwrap();
    writeln!(file, "name,age,email").unwrap();
    writeln!(file, "Alice,30,alice@example.com").unwrap();
    writeln!(file, "Bob,25,bob@example.com").unwrap();
    writeln!(file, "Charlie,40,charlie@example.com").unwrap();
    writeln!(file, "Diana,35,diana@example.com").unwrap();
    writeln!(file, "Eve,28,eve@example.com").unwrap();
    file.flush().unwrap();

    let path = file.path().to_str().unwrap();
    let count = import_file(&pool, path, ImportFormat::Csv, "main", "users")
        .await
        .unwrap();

    assert_eq!(count, 5);

    let rows: Vec<(String, String, String)> =
        sqlx::query_as("SELECT name, age, email FROM users ORDER BY name")
            .fetch_all(&sq)
            .await
            .unwrap();

    assert_eq!(rows.len(), 5);
    assert_eq!(rows[0], ("Alice".into(), "30".into(), "alice@example.com".into()));
    assert_eq!(rows[1], ("Bob".into(), "25".into(), "bob@example.com".into()));
    assert_eq!(rows[4], ("Eve".into(), "28".into(), "eve@example.com".into()));
}

#[tokio::test]
async fn test_import_csv_large_batch() {
    let (sq, pool) = setup_sqlite_with_table().await;

    let mut file = tempfile::NamedTempFile::new().unwrap();
    writeln!(file, "name,age,email").unwrap();
    for i in 0..250 {
        writeln!(file, "user_{i},{i},user_{i}@test.com").unwrap();
    }
    file.flush().unwrap();

    let path = file.path().to_str().unwrap();
    let count = import_file(&pool, path, ImportFormat::Csv, "main", "users")
        .await
        .unwrap();

    assert_eq!(count, 250);

    let row_count: i32 = sqlx::query("SELECT COUNT(*) as c FROM users")
        .fetch_one(&sq)
        .await
        .unwrap()
        .get("c");
    assert_eq!(row_count, 250);
}

#[tokio::test]
async fn test_import_csv_special_chars() {
    let (sq, pool) = setup_sqlite_with_table().await;

    let mut file = tempfile::NamedTempFile::new().unwrap();
    writeln!(file, "name,age,email").unwrap();
    // Comma inside a quoted field
    writeln!(file, r#""Last, First",30,test@example.com"#).unwrap();
    // Single quote in value (SQL injection-like)
    writeln!(file, r#"O'Brien,40,obrien@example.com"#).unwrap();
    // Double quotes inside a quoted field: CSV escapes " as ""
    file.write_all(b"\"\"\"Nickname\"\" Bob\",25,bob@example.com\n").unwrap();
    // Newline inside a quoted field
    write!(file, "\"Line1\nLine2\",50,multi@example.com\n").unwrap();
    file.flush().unwrap();

    let path = file.path().to_str().unwrap();
    let count = import_file(&pool, path, ImportFormat::Csv, "main", "users")
        .await
        .unwrap();

    assert_eq!(count, 4);

    let rows: Vec<(String, String, String)> =
        sqlx::query_as("SELECT name, age, email FROM users ORDER BY age")
            .fetch_all(&sq)
            .await
            .unwrap();

    assert_eq!(rows.len(), 4);

    // Verify comma in value was preserved
    let comma_row = rows.iter().find(|r| r.1 == "30").unwrap();
    assert_eq!(comma_row.0, "Last, First");

    // Verify single quote was preserved
    let quote_row = rows.iter().find(|r| r.1 == "40").unwrap();
    assert_eq!(quote_row.0, "O'Brien");

    // Verify double quotes in value
    let dquote_row = rows.iter().find(|r| r.1 == "25").unwrap();
    assert_eq!(dquote_row.0, "\"Nickname\" Bob");

    // Verify embedded newline was preserved
    let newline_row = rows.iter().find(|r| r.1 == "50").unwrap();
    assert_eq!(newline_row.0, "Line1\nLine2");
}

#[tokio::test]
async fn test_import_csv_empty() {
    let (_sq, pool) = setup_sqlite_with_table().await;

    let mut file = tempfile::NamedTempFile::new().unwrap();
    writeln!(file, "name,age,email").unwrap();
    file.flush().unwrap();

    let path = file.path().to_str().unwrap();
    let count = import_file(&pool, path, ImportFormat::Csv, "main", "users")
        .await
        .unwrap();

    assert_eq!(count, 0);
}

// ---------------------------------------------------------------------------
// JSON tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_import_json_array() {
    let (sq, pool) = setup_sqlite_with_table().await;

    let mut file = tempfile::NamedTempFile::new().unwrap();
    write!(
        file,
        r#"[
            {{"name":"Alice","age":"30","email":"a@b.com"}},
            {{"name":"Bob","age":"25","email":"b@b.com"}},
            {{"name":"Charlie","age":"40","email":"c@b.com"}}
        ]"#
    )
    .unwrap();
    file.flush().unwrap();

    let path = file.path().to_str().unwrap();
    let count = import_file(&pool, path, ImportFormat::Json, "main", "users")
        .await
        .unwrap();

    assert_eq!(count, 3);

    let rows: Vec<(String, String, String)> =
        sqlx::query_as("SELECT name, age, email FROM users ORDER BY name")
            .fetch_all(&sq)
            .await
            .unwrap();

    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0], ("Alice".into(), "30".into(), "a@b.com".into()));
    assert_eq!(rows[1], ("Bob".into(), "25".into(), "b@b.com".into()));
    assert_eq!(rows[2], ("Charlie".into(), "40".into(), "c@b.com".into()));
}

#[tokio::test]
async fn test_import_json_empty() {
    let (_sq, pool) = setup_sqlite_with_table().await;

    let mut file = tempfile::NamedTempFile::new().unwrap();
    write!(file, "[]").unwrap();
    file.flush().unwrap();

    let path = file.path().to_str().unwrap();
    let count = import_file(&pool, path, ImportFormat::Json, "main", "users")
        .await
        .unwrap();

    assert_eq!(count, 0);
}

// ---------------------------------------------------------------------------
// Unsupported backend
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_import_unsupported_backend() {
    // DynamoDB requires only a client, which we can construct without a real endpoint.
    let config = aws_sdk_dynamodb::Config::builder()
        .behavior_version_latest()
        .region(aws_sdk_dynamodb::config::Region::new("us-east-1"))
        .build();
    let client = aws_sdk_dynamodb::Client::from_conf(config);
    let pool = DbPool::DynamoDb(Box::new(client));

    let mut file = tempfile::NamedTempFile::new().unwrap();
    writeln!(file, "name,age,email").unwrap();
    writeln!(file, "Alice,30,alice@example.com").unwrap();
    file.flush().unwrap();

    let path = file.path().to_str().unwrap();
    let result = import_file(&pool, path, ImportFormat::Csv, "main", "users").await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("not supported"),
        "Expected 'not supported' in error, got: {err_msg}"
    );
}
