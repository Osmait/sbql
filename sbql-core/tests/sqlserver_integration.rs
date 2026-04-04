use sbql_core::{
    query::execute_page,
    schema::{
        execute_cell_update, execute_row_delete, get_primary_keys, list_tables, load_diagram,
    },
    DbPool,
};
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::mssql_server::MssqlServer;

const SA_PASSWORD: &str = "YourStrong@Passw0rd";

async fn make_pool(
    host: &str,
    port: u16,
    database: &str,
) -> bb8::Pool<bb8_tiberius::ConnectionManager> {
    let mut config = tiberius::Config::new();
    config.host(host);
    config.port(port);
    config.database(database);
    config.authentication(tiberius::AuthMethod::sql_server("sa", SA_PASSWORD));
    config.trust_cert();

    let mgr = bb8_tiberius::ConnectionManager::new(config);
    bb8::Pool::builder()
        .max_size(5)
        .build(mgr)
        .await
        .unwrap()
}

/// All SQL Server tests run against a single container to avoid memory pressure.
/// SQL Server requires at least 2GB RAM per instance.
#[tokio::test]
async fn test_sqlserver_integration() {
    // --- Start container ---
    let container = MssqlServer::default()
        .with_accept_eula()
        .with_sa_password(SA_PASSWORD)
        .start()
        .await
        .unwrap();

    let host = container.get_host().await.unwrap().to_string();
    let port = container.get_host_port_ipv4(1433).await.unwrap();

    // Create the test database via the master pool
    let master_pool = make_pool(&host, port, "master").await;
    {
        let mut conn = master_pool.get().await.unwrap();
        conn.execute("CREATE DATABASE testdb", &[]).await.unwrap();
    }

    // Pool connected to testdb
    let raw = make_pool(&host, port, "testdb").await;
    let pool = DbPool::SqlServer(Box::new(raw.clone()));

    // Seed tables and data
    {
        let mut conn = raw.get().await.unwrap();
        conn.execute(
            "CREATE TABLE users (
                id INT IDENTITY(1,1) PRIMARY KEY,
                username NVARCHAR(50) NOT NULL,
                active BIT DEFAULT 1
            )",
            &[],
        )
        .await
        .unwrap();

        conn.execute(
            "CREATE TABLE posts (
                id INT IDENTITY(1,1) PRIMARY KEY,
                user_id INT NOT NULL FOREIGN KEY REFERENCES users(id),
                title NVARCHAR(100) NOT NULL
            )",
            &[],
        )
        .await
        .unwrap();

        conn.execute(
            "INSERT INTO users (username) VALUES ('alice'), ('bob')",
            &[],
        )
        .await
        .unwrap();

        conn.execute(
            "INSERT INTO posts (user_id, title) VALUES (1, 'Hello'), (2, 'World')",
            &[],
        )
        .await
        .unwrap();
    }

    // --- 1. list_tables ---
    {
        let tables = list_tables(&pool).await.expect("Failed to list tables");
        let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
        assert!(
            names.contains(&"users"),
            "Expected 'users' table, got: {names:?}"
        );
        assert!(
            names.contains(&"posts"),
            "Expected 'posts' table, got: {names:?}"
        );
    }

    // --- 2. get_primary_keys ---
    {
        let pk_users = get_primary_keys(&pool, "dbo", "users")
            .await
            .expect("Failed to get PKs for users");
        assert_eq!(pk_users, vec!["id".to_string()]);

        let pk_posts = get_primary_keys(&pool, "dbo", "posts")
            .await
            .expect("Failed to get PKs for posts");
        assert_eq!(pk_posts, vec!["id".to_string()]);
    }

    // --- 3. execute_page ---
    {
        let result = execute_page(&pool, "SELECT * FROM users", 0)
            .await
            .expect("Failed to execute page");
        assert_eq!(result.rows.len(), 2);
        assert!(result.columns.contains(&"username".to_string()));
    }

    // --- 4. pagination ---
    {
        let mut conn = raw.get().await.unwrap();
        conn.execute(
            "CREATE TABLE many_rows (id INT IDENTITY(1,1) PRIMARY KEY, val NVARCHAR(50))",
            &[],
        )
        .await
        .unwrap();

        for i in 0..150 {
            conn.execute(
                "INSERT INTO many_rows (val) VALUES (@P1)",
                &[&format!("row_{i}")],
            )
            .await
            .unwrap();
        }

        // SQL Server disallows ORDER BY inside derived tables without TOP/OFFSET,
        // so the pagination wrapper uses ROW_NUMBER() over (SELECT NULL). Use a
        // plain SELECT here and let the engine handle ordering via ROW_NUMBER.
        let page0 = execute_page(&pool, "SELECT * FROM many_rows", 0)
            .await
            .unwrap();
        assert_eq!(page0.rows.len(), 100);
        assert!(page0.has_next_page);
        assert_eq!(page0.page, 0);

        let page1 = execute_page(&pool, "SELECT * FROM many_rows", 1)
            .await
            .unwrap();
        assert_eq!(page1.rows.len(), 50);
        assert!(!page1.has_next_page);
        assert_eq!(page1.page, 1);
    }

    // --- 5. diagram ---
    {
        let diagram = load_diagram(&pool).await.expect("Failed to load diagram");

        let users_table = diagram
            .tables
            .iter()
            .find(|t| t.name == "users")
            .expect("Users table missing in diagram");
        assert!(
            users_table
                .columns
                .iter()
                .any(|c| c.name == "id" && c.is_pk),
            "Expected id column to be PK"
        );

        let fk = diagram
            .foreign_keys
            .iter()
            .find(|fk| fk.from_table == "posts" && fk.to_table == "users")
            .expect("Foreign key from posts to users missing");
        assert_eq!(fk.from_col, "user_id");
        assert_eq!(fk.to_col, "id");
    }

    // --- 6. cell update ---
    {
        execute_cell_update(&pool, "dbo", "users", "id", "1", "username", "alice_updated")
            .await
            .expect("Failed to update cell");

        let mut conn = raw.get().await.unwrap();
        let stream = conn
            .query("SELECT username FROM users WHERE id = 1", &[])
            .await
            .unwrap();
        let row = stream.into_row().await.unwrap().unwrap();
        let username: &str = row.get::<&str, _>("username").unwrap();
        assert_eq!(username, "alice_updated");
    }

    // --- 7. row delete ---
    {
        execute_row_delete(&pool, "dbo", "posts", "id", "2")
            .await
            .expect("Failed to delete row");

        let mut conn = raw.get().await.unwrap();
        let stream = conn
            .query("SELECT COUNT(*) AS cnt FROM posts WHERE id = 2", &[])
            .await
            .unwrap();
        let row = stream.into_row().await.unwrap().unwrap();
        let count: i32 = row.get::<i32, _>("cnt").unwrap();
        assert_eq!(count, 0);
    }

    // --- 8. types ---
    {
        let mut conn = raw.get().await.unwrap();
        conn.execute(
            "CREATE TABLE type_test (
                id INT IDENTITY(1,1) PRIMARY KEY,
                int_val INT,
                big_val BIGINT,
                float_val FLOAT,
                bit_val BIT,
                nvar_val NVARCHAR(100),
                dt_val DATETIME2,
                dec_val DECIMAL(10,2)
            )",
            &[],
        )
        .await
        .unwrap();

        conn.execute(
            "INSERT INTO type_test (int_val, big_val, float_val, bit_val, nvar_val, dt_val, dec_val)
             VALUES (42, 9999999999, 3.14, 1, N'hello world', '2024-01-15T10:30:00', 123.45)",
            &[],
        )
        .await
        .unwrap();

        let result = execute_page(&pool, "SELECT * FROM type_test", 0)
            .await
            .expect("Failed to execute type_test query");
        assert_eq!(result.rows.len(), 1);

        let row = &result.rows[0];
        let cols = &result.columns;

        let int_idx = cols.iter().position(|c| c == "int_val").unwrap();
        assert_eq!(row[int_idx], "42");

        let big_idx = cols.iter().position(|c| c == "big_val").unwrap();
        assert_eq!(row[big_idx], "9999999999");

        let float_idx = cols.iter().position(|c| c == "float_val").unwrap();
        assert!(
            row[float_idx].starts_with("3.14"),
            "Expected float to start with 3.14, got: {}",
            row[float_idx]
        );

        let bit_idx = cols.iter().position(|c| c == "bit_val").unwrap();
        assert!(
            row[bit_idx] == "true" || row[bit_idx] == "1",
            "Expected bit_val to be 'true' or '1', got: {}",
            row[bit_idx]
        );

        let nvar_idx = cols.iter().position(|c| c == "nvar_val").unwrap();
        assert_eq!(row[nvar_idx], "hello world");

        let dt_idx = cols.iter().position(|c| c == "dt_val").unwrap();
        assert!(
            row[dt_idx].contains("2024-01-15"),
            "Expected datetime to contain '2024-01-15', got: {}",
            row[dt_idx]
        );

        let dec_idx = cols.iter().position(|c| c == "dec_val").unwrap();
        assert!(
            row[dec_idx].starts_with("123.45"),
            "Expected decimal to start with '123.45', got: {}",
            row[dec_idx]
        );
    }

    // --- 9. unicode (emoji + CJK) ---
    {
        let mut conn = raw.get().await.unwrap();
        conn.execute(
            "CREATE TABLE unicode_test (
                id INT IDENTITY(1,1) PRIMARY KEY,
                label NVARCHAR(200)
            )",
            &[],
        )
        .await
        .unwrap();

        let emoji_cjk = "Hello \u{1F600} \u{4E16}\u{754C}"; // Hello 😀 世界
        conn.execute(
            "INSERT INTO unicode_test (label) VALUES (@P1)",
            &[&emoji_cjk],
        )
        .await
        .unwrap();

        let result = execute_page(&pool, "SELECT * FROM unicode_test", 0)
            .await
            .expect("Failed to execute unicode_test query");
        assert_eq!(result.rows.len(), 1);

        let label_idx = result
            .columns
            .iter()
            .position(|c| c == "label")
            .unwrap();
        assert_eq!(
            result.rows[0][label_idx], emoji_cjk,
            "Unicode roundtrip failed"
        );
    }

    // --- 10. null handling ---
    {
        let mut conn = raw.get().await.unwrap();
        conn.execute(
            "CREATE TABLE null_test (
                id INT IDENTITY(1,1) PRIMARY KEY,
                int_col INT,
                nvar_col NVARCHAR(50),
                dt_col DATETIME2,
                bit_col BIT
            )",
            &[],
        )
        .await
        .unwrap();

        conn.execute(
            "INSERT INTO null_test (int_col, nvar_col, dt_col, bit_col)
             VALUES (NULL, NULL, NULL, NULL)",
            &[],
        )
        .await
        .unwrap();

        let result = execute_page(&pool, "SELECT * FROM null_test", 0)
            .await
            .expect("Failed to execute null_test query");
        assert_eq!(result.rows.len(), 1);

        let row = &result.rows[0];
        let cols = &result.columns;

        // All nullable columns should round-trip as NULL (displayed as empty or "NULL")
        for col_name in &["int_col", "nvar_col", "dt_col", "bit_col"] {
            let idx = cols.iter().position(|c| c == col_name).unwrap();
            assert!(
                row[idx].is_empty() || row[idx] == "NULL",
                "Expected NULL representation for {col_name}, got: '{}'",
                row[idx]
            );
        }
    }

    // --- 11. large text (nvarchar(max) with 10KB) ---
    {
        let mut conn = raw.get().await.unwrap();
        conn.execute(
            "CREATE TABLE large_text_test (
                id INT IDENTITY(1,1) PRIMARY KEY,
                content NVARCHAR(MAX)
            )",
            &[],
        )
        .await
        .unwrap();

        let large_text: String = "A".repeat(10 * 1024); // 10KB
        conn.execute(
            "INSERT INTO large_text_test (content) VALUES (@P1)",
            &[&large_text.as_str()],
        )
        .await
        .unwrap();

        let result = execute_page(&pool, "SELECT * FROM large_text_test", 0)
            .await
            .expect("Failed to execute large_text_test query");
        assert_eq!(result.rows.len(), 1);

        let content_idx = result
            .columns
            .iter()
            .position(|c| c == "content")
            .unwrap();
        assert_eq!(
            result.rows[0][content_idx].len(),
            10 * 1024,
            "Large text length mismatch"
        );
        assert_eq!(result.rows[0][content_idx], large_text);
    }

    // --- 12. empty result set ---
    {
        let mut conn = raw.get().await.unwrap();
        conn.execute(
            "CREATE TABLE empty_test (
                id INT IDENTITY(1,1) PRIMARY KEY,
                val NVARCHAR(50)
            )",
            &[],
        )
        .await
        .unwrap();

        let result = execute_page(&pool, "SELECT * FROM empty_test", 0)
            .await
            .expect("Failed to execute empty_test query");
        assert!(
            result.rows.is_empty(),
            "Expected empty result set, got {} rows",
            result.rows.len()
        );
        assert!(!result.has_next_page);
    }

    // --- 13. composite primary key ---
    {
        let mut conn = raw.get().await.unwrap();
        conn.execute(
            "CREATE TABLE composite_pk_test (
                tenant_id INT NOT NULL,
                item_id INT NOT NULL,
                label NVARCHAR(50),
                PRIMARY KEY (tenant_id, item_id)
            )",
            &[],
        )
        .await
        .unwrap();

        conn.execute(
            "INSERT INTO composite_pk_test (tenant_id, item_id, label)
             VALUES (1, 100, 'first'), (1, 200, 'second'), (2, 100, 'third')",
            &[],
        )
        .await
        .unwrap();

        let pks = get_primary_keys(&pool, "dbo", "composite_pk_test")
            .await
            .expect("Failed to get composite PKs");
        assert_eq!(pks.len(), 2, "Expected 2 PK columns, got: {pks:?}");
        assert!(
            pks.contains(&"tenant_id".to_string()),
            "Expected tenant_id in PKs, got: {pks:?}"
        );
        assert!(
            pks.contains(&"item_id".to_string()),
            "Expected item_id in PKs, got: {pks:?}"
        );

        let result = execute_page(&pool, "SELECT * FROM composite_pk_test", 0)
            .await
            .expect("Failed to execute composite_pk_test query");
        assert_eq!(result.rows.len(), 3);
    }
}
