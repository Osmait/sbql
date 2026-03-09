use sbql_core::{
    query::execute_page,
    schema::{
        execute_cell_update, execute_row_delete, get_primary_keys, list_tables, load_diagram,
    },
    DbPool,
};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Row, SqlitePool};

/// Helper: create an in-memory SQLite pool with FK support and test tables.
async fn setup_sqlite() -> (SqlitePool, DbPool) {
    let sq = SqlitePoolOptions::new()
        .max_connections(1)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("PRAGMA foreign_keys = ON")
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite");

    sqlx::query(
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE,
            active INTEGER DEFAULT 1
        )",
    )
    .execute(&sq)
    .await
    .expect("Failed to create users table");

    sqlx::query(
        "CREATE TABLE posts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL REFERENCES users(id),
            title TEXT NOT NULL,
            content TEXT
        )",
    )
    .execute(&sq)
    .await
    .expect("Failed to create posts table");

    sqlx::query("INSERT INTO users (username) VALUES ('alice'), ('bob')")
        .execute(&sq)
        .await
        .expect("Failed to insert users");

    sqlx::query(
        "INSERT INTO posts (user_id, title, content) VALUES
        (1, 'Alice First Post', 'Hello world'),
        (2, 'Bob Post', 'Hi there')",
    )
    .execute(&sq)
    .await
    .expect("Failed to insert posts");

    let pool = DbPool::Sqlite(sq.clone());
    (sq, pool)
}

#[tokio::test]
async fn test_sqlite_list_tables() {
    let (_sq, pool) = setup_sqlite().await;
    let tables = list_tables(&pool).await.expect("list_tables failed");
    let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"users"), "Expected 'users' table");
    assert!(names.contains(&"posts"), "Expected 'posts' table");
    for t in &tables {
        assert_eq!(t.schema, "main");
    }
}

#[tokio::test]
async fn test_sqlite_get_primary_keys() {
    let (_sq, pool) = setup_sqlite().await;
    let pks = get_primary_keys(&pool, "main", "users")
        .await
        .expect("get_primary_keys failed");
    assert_eq!(pks, vec!["id".to_string()]);
}

#[tokio::test]
async fn test_sqlite_execute_page() {
    let (_sq, pool) = setup_sqlite().await;
    let result = execute_page(&pool, "SELECT * FROM users ORDER BY id", 0)
        .await
        .expect("execute_page failed");
    assert_eq!(result.rows.len(), 2);
    assert!(result.columns.contains(&"id".to_string()));
    assert!(result.columns.contains(&"username".to_string()));
    assert!(result.columns.contains(&"active".to_string()));
}

#[tokio::test]
async fn test_sqlite_diagram() {
    let (_sq, pool) = setup_sqlite().await;
    let diagram = load_diagram(&pool).await.expect("load_diagram failed");

    let user_table = diagram
        .tables
        .iter()
        .find(|t| t.name == "users")
        .expect("users table missing in diagram");
    assert_eq!(user_table.columns.len(), 3);
    assert!(user_table.columns.iter().any(|c| c.name == "id" && c.is_pk));

    // Check foreign key
    let post_fk = diagram
        .foreign_keys
        .iter()
        .find(|fk| fk.from_table == "posts" && fk.to_table == "users")
        .expect("FK from posts to users missing");
    assert_eq!(post_fk.from_col, "user_id");
    assert_eq!(post_fk.to_col, "id");
}

#[tokio::test]
async fn test_sqlite_cell_update() {
    let (sq, pool) = setup_sqlite().await;
    execute_cell_update(
        &pool,
        "main",
        "users",
        "id",
        "1",
        "username",
        "alice_updated",
    )
    .await
    .expect("cell update failed");

    let updated: String = sqlx::query("SELECT username FROM users WHERE id = 1")
        .fetch_one(&sq)
        .await
        .unwrap()
        .get("username");
    assert_eq!(updated, "alice_updated");
}

#[tokio::test]
async fn test_sqlite_row_delete() {
    let (sq, pool) = setup_sqlite().await;
    execute_row_delete(&pool, "main", "posts", "id", "2")
        .await
        .expect("row delete failed");

    let count: i32 = sqlx::query("SELECT COUNT(*) as c FROM posts WHERE id = 2")
        .fetch_one(&sq)
        .await
        .unwrap()
        .get("c");
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_sqlite_pagination() {
    let sq = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    sqlx::query("CREATE TABLE many_rows (id INTEGER PRIMARY KEY AUTOINCREMENT, val TEXT)")
        .execute(&sq)
        .await
        .unwrap();

    for i in 0..150 {
        sqlx::query("INSERT INTO many_rows (val) VALUES ($1)")
            .bind(format!("row_{i}"))
            .execute(&sq)
            .await
            .unwrap();
    }

    let pool = DbPool::Sqlite(sq);

    let page0 = execute_page(&pool, "SELECT * FROM many_rows ORDER BY id", 0)
        .await
        .unwrap();
    assert_eq!(page0.rows.len(), 100);
    assert!(page0.has_next_page);
    assert_eq!(page0.page, 0);

    let page1 = execute_page(&pool, "SELECT * FROM many_rows ORDER BY id", 1)
        .await
        .unwrap();
    assert_eq!(page1.rows.len(), 50);
    assert!(!page1.has_next_page);
    assert_eq!(page1.page, 1);
}

#[tokio::test]
async fn test_sqlite_types() {
    let sq = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    sqlx::query(
        "CREATE TABLE type_test (
            id INTEGER PRIMARY KEY,
            text_val TEXT,
            real_val REAL,
            blob_val BLOB,
            null_val TEXT
        )",
    )
    .execute(&sq)
    .await
    .unwrap();

    sqlx::query("INSERT INTO type_test (id, text_val, real_val, blob_val, null_val) VALUES (1, 'hello', 3.14, X'DEADBEEF', NULL)")
        .execute(&sq)
        .await
        .unwrap();

    let pool = DbPool::Sqlite(sq);
    let result = execute_page(&pool, "SELECT * FROM type_test", 0)
        .await
        .unwrap();

    assert_eq!(result.rows.len(), 1);
    let row = &result.rows[0];
    let cols = &result.columns;

    let text_idx = cols.iter().position(|c| c == "text_val").unwrap();
    assert_eq!(row[text_idx], "hello");

    let real_idx = cols.iter().position(|c| c == "real_val").unwrap();
    assert!(row[real_idx].starts_with("3.14"));

    let blob_idx = cols.iter().position(|c| c == "blob_val").unwrap();
    assert_eq!(row[blob_idx], "\\xdeadbeef");

    let null_idx = cols.iter().position(|c| c == "null_val").unwrap();
    assert_eq!(row[null_idx], ""); // NULL → empty string
}

#[tokio::test]
async fn test_sqlite_empty_result() {
    let sq = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    sqlx::query("CREATE TABLE empty_table (id INTEGER PRIMARY KEY, name TEXT)")
        .execute(&sq)
        .await
        .unwrap();

    let pool = DbPool::Sqlite(sq);
    let result = execute_page(&pool, "SELECT * FROM empty_table", 0)
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 0);
    assert!(!result.has_next_page);
    assert!(result.columns.is_empty());
}
