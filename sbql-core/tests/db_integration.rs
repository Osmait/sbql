use sbql_core::{
    query::execute_page,
    schema::{
        execute_cell_update, execute_row_delete, get_primary_keys, list_tables, load_diagram,
    },
};
use sqlx::{PgPool, Row};
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

#[tokio::test]
async fn test_database_schema_and_mutations() {
    // 1. Start a PostgreSQL container using testcontainers
    let container = Postgres::default().start().await.unwrap();
    let host_ip = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(5432).await.unwrap();

    // The default testcontainers-modules postgres setup uses:
    // user: postgres, db: postgres, password: postgres
    let connection_string = format!(
        "postgresql://postgres:postgres@{}:{}/postgres",
        host_ip, host_port
    );

    // 2. Connect to the database using sqlx
    let pool = PgPool::connect(&connection_string)
        .await
        .expect("Failed to connect to DB");

    // 3. Setup test schema and data
    sqlx::query(
        "CREATE TABLE users (
            id SERIAL PRIMARY KEY,
            username VARCHAR(50) NOT NULL UNIQUE,
            active BOOLEAN DEFAULT true
        );",
    )
    .execute(&pool)
    .await
    .expect("Failed to create users table");

    sqlx::query(
        "CREATE TABLE posts (
            id SERIAL PRIMARY KEY,
            user_id INTEGER NOT NULL REFERENCES users(id),
            title VARCHAR(100) NOT NULL,
            content TEXT
        );",
    )
    .execute(&pool)
    .await
    .expect("Failed to create posts table");

    sqlx::query("CREATE TYPE status_enum AS ENUM ('new', 'active', 'closed');")
        .execute(&pool)
        .await
        .expect("Failed to create enum");

    sqlx::query(
        "CREATE TABLE complex_types (
            id SERIAL PRIMARY KEY,
            js_data JSONB,
            tags TEXT[],
            uid UUID,
            created_at TIMESTAMPTZ,
            raw_bytes BYTEA,
            status status_enum
        );",
    )
    .execute(&pool)
    .await
    .expect("Failed to create complex types table");

    sqlx::query("INSERT INTO users (username) VALUES ('alice'), ('bob');")
        .execute(&pool)
        .await
        .expect("Failed to insert users");

    sqlx::query(
        "INSERT INTO posts (user_id, title, content) VALUES 
        (1, 'Alice First Post', 'Hello world'),
        (2, 'Bob Post', 'Hi there');",
    )
    .execute(&pool)
    .await
    .expect("Failed to insert posts");

    sqlx::query(
        "INSERT INTO complex_types (js_data, tags, uid, created_at, raw_bytes, status) VALUES 
        ('{\"key\": \"value\"}', ARRAY['rust', 'sql'], '123e4567-e89b-12d3-a456-426614174000', '2026-03-06T12:00:00Z', '\\xDEADBEEF', 'active');"
    )
    .execute(&pool)
    .await
    .expect("Failed to insert complex types");

    // 4. Test complex data types parsing
    let res = execute_page(&pool, "SELECT * FROM complex_types ORDER BY id", 0)
        .await
        .expect("Failed to execute complex query");
    assert_eq!(res.rows.len(), 1);
    let row = &res.rows[0];

    // Check specific columns - checking our `pg_value_to_string` implementation
    let cols = &res.columns;
    let js_idx = cols.iter().position(|c| c == "js_data").unwrap();
    assert_eq!(row[js_idx], "{\"key\":\"value\"}"); // JSONB formatting

    let tags_idx = cols.iter().position(|c| c == "tags").unwrap();
    assert_eq!(row[tags_idx], "{rust,sql}");

    let uid_idx = cols.iter().position(|c| c == "uid").unwrap();
    assert_eq!(row[uid_idx], "123e4567-e89b-12d3-a456-426614174000");

    let created_idx = cols.iter().position(|c| c == "created_at").unwrap();
    assert!(row[created_idx].starts_with("2026-03-06T12:00:00"));

    let bytes_idx = cols.iter().position(|c| c == "raw_bytes").unwrap();
    assert_eq!(row[bytes_idx], "\\xdeadbeef");

    let status_idx = cols.iter().position(|c| c == "status").unwrap();
    assert_eq!(row[status_idx], "active"); // Enum fallback check

    // 4. Test: list_tables
    let tables = list_tables(&pool).await.expect("Failed to list tables");
    let table_names: Vec<String> = tables.into_iter().map(|t| t.name).collect();
    assert!(table_names.contains(&"users".to_string()));
    assert!(table_names.contains(&"posts".to_string()));

    // 5. Test: get_primary_keys
    let pk_users = get_primary_keys(&pool, "public", "users")
        .await
        .expect("Failed to get PKs");
    assert_eq!(pk_users, vec!["id".to_string()]);

    let pk_posts = get_primary_keys(&pool, "public", "posts")
        .await
        .expect("Failed to get PKs");
    assert_eq!(pk_posts, vec!["id".to_string()]);

    // 6. Test: load_diagram
    let diagram = load_diagram(&pool).await.expect("Failed to load diagram");

    // Check tables in diagram
    let users_schema = diagram
        .tables
        .iter()
        .find(|t| t.name == "users")
        .expect("Users table missing in diagram");
    assert_eq!(users_schema.columns.len(), 3);
    assert!(users_schema
        .columns
        .iter()
        .any(|c| c.name == "id" && c.is_pk));

    // Check foreign keys in diagram
    let post_fk = diagram
        .foreign_keys
        .iter()
        .find(|fk| fk.from_table == "posts" && fk.to_table == "users")
        .expect("Foreign key missing");
    assert_eq!(post_fk.from_col, "user_id");
    assert_eq!(post_fk.to_col, "id");

    // 7. Test: execute_cell_update
    // Update Alice's username to 'alice_updated'
    execute_cell_update(
        &pool,
        "public",
        "users",
        "id",
        "1", // Alice's ID is 1 (SERIAL starts at 1)
        "username",
        "alice_updated",
    )
    .await
    .expect("Failed to execute cell update");

    // Verify the update
    let updated_username: String = sqlx::query("SELECT username FROM users WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("username");
    assert_eq!(updated_username, "alice_updated");

    // 8. Test: execute_row_delete
    // Delete Bob's post (ID 2)
    execute_row_delete(&pool, "public", "posts", "id", "2")
        .await
        .expect("Failed to delete row");

    // Verify the deletion
    let post_count: i64 = sqlx::query("SELECT COUNT(*) FROM posts WHERE id = 2")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get(0);
    assert_eq!(post_count, 0);
}

#[tokio::test]
async fn test_execute_page_pagination() {
    let container = Postgres::default().start().await.unwrap();
    let host_ip = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(5432).await.unwrap();
    let connection_string = format!(
        "postgresql://postgres:postgres@{}:{}/postgres",
        host_ip, host_port
    );
    let pool = PgPool::connect(&connection_string).await.unwrap();

    // Create a table with many rows to test pagination
    sqlx::query("CREATE TABLE many_rows (id SERIAL PRIMARY KEY, val TEXT);")
        .execute(&pool)
        .await
        .unwrap();

    // Insert 150 rows to exceed PAGE_SIZE (100)
    for i in 0..150 {
        sqlx::query("INSERT INTO many_rows (val) VALUES ($1)")
            .bind(format!("row_{}", i))
            .execute(&pool)
            .await
            .unwrap();
    }

    // Page 0 should have 100 rows and has_next_page=true
    let page0 = execute_page(&pool, "SELECT * FROM many_rows ORDER BY id", 0)
        .await
        .unwrap();
    assert_eq!(page0.rows.len(), 100);
    assert!(page0.has_next_page);
    assert_eq!(page0.page, 0);

    // Page 1 should have 50 rows and has_next_page=false
    let page1 = execute_page(&pool, "SELECT * FROM many_rows ORDER BY id", 1)
        .await
        .unwrap();
    assert_eq!(page1.rows.len(), 50);
    assert!(!page1.has_next_page);
    assert_eq!(page1.page, 1);
}

#[tokio::test]
async fn test_execute_page_empty_result() {
    let container = Postgres::default().start().await.unwrap();
    let host_ip = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(5432).await.unwrap();
    let connection_string = format!(
        "postgresql://postgres:postgres@{}:{}/postgres",
        host_ip, host_port
    );
    let pool = PgPool::connect(&connection_string).await.unwrap();

    sqlx::query("CREATE TABLE empty_table (id SERIAL PRIMARY KEY, name TEXT);")
        .execute(&pool)
        .await
        .unwrap();

    let result = execute_page(&pool, "SELECT * FROM empty_table", 0)
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 0);
    assert!(!result.has_next_page);
    assert!(result.columns.is_empty());
}

#[tokio::test]
async fn test_boolean_and_numeric_types() {
    let container = Postgres::default().start().await.unwrap();
    let host_ip = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(5432).await.unwrap();
    let connection_string = format!(
        "postgresql://postgres:postgres@{}:{}/postgres",
        host_ip, host_port
    );
    let pool = PgPool::connect(&connection_string).await.unwrap();

    sqlx::query(
        "CREATE TABLE type_test (
            id SERIAL PRIMARY KEY,
            bool_val BOOLEAN,
            small_val SMALLINT,
            int_val INTEGER,
            big_val BIGINT,
            real_val REAL,
            double_val DOUBLE PRECISION,
            numeric_val NUMERIC(10,2),
            null_text TEXT
        );",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO type_test (bool_val, small_val, int_val, big_val, real_val, double_val, numeric_val, null_text)
         VALUES (true, 42, 1000, 9999999999, 3.14, 2.718281828, 123.45, NULL);"
    )
    .execute(&pool)
    .await
    .unwrap();

    let result = execute_page(&pool, "SELECT * FROM type_test ORDER BY id", 0)
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 1);
    let row = &result.rows[0];
    let cols = &result.columns;

    let bool_idx = cols.iter().position(|c| c == "bool_val").unwrap();
    assert_eq!(row[bool_idx], "true");

    let small_idx = cols.iter().position(|c| c == "small_val").unwrap();
    assert_eq!(row[small_idx], "42");

    let int_idx = cols.iter().position(|c| c == "int_val").unwrap();
    assert_eq!(row[int_idx], "1000");

    let big_idx = cols.iter().position(|c| c == "big_val").unwrap();
    assert_eq!(row[big_idx], "9999999999");

    let null_idx = cols.iter().position(|c| c == "null_text").unwrap();
    assert_eq!(row[null_idx], ""); // NULL → empty string
}

#[tokio::test]
async fn test_date_time_types() {
    let container = Postgres::default().start().await.unwrap();
    let host_ip = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(5432).await.unwrap();
    let connection_string = format!(
        "postgresql://postgres:postgres@{}:{}/postgres",
        host_ip, host_port
    );
    let pool = PgPool::connect(&connection_string).await.unwrap();

    sqlx::query(
        "CREATE TABLE time_test (
            id SERIAL PRIMARY KEY,
            ts TIMESTAMP,
            d DATE,
            t TIME
        );",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO time_test (ts, d, t) VALUES ('2024-01-15 10:30:00', '2024-01-15', '10:30:00');"
    )
    .execute(&pool)
    .await
    .unwrap();

    let result = execute_page(&pool, "SELECT * FROM time_test ORDER BY id", 0)
        .await
        .unwrap();
    let row = &result.rows[0];
    let cols = &result.columns;

    let ts_idx = cols.iter().position(|c| c == "ts").unwrap();
    assert!(row[ts_idx].contains("2024-01-15"));

    let d_idx = cols.iter().position(|c| c == "d").unwrap();
    assert_eq!(row[d_idx], "2024-01-15");

    let t_idx = cols.iter().position(|c| c == "t").unwrap();
    assert!(row[t_idx].contains("10:30:00"));
}

#[tokio::test]
async fn test_array_types() {
    let container = Postgres::default().start().await.unwrap();
    let host_ip = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(5432).await.unwrap();
    let connection_string = format!(
        "postgresql://postgres:postgres@{}:{}/postgres",
        host_ip, host_port
    );
    let pool = PgPool::connect(&connection_string).await.unwrap();

    sqlx::query(
        "CREATE TABLE array_test (
            id SERIAL PRIMARY KEY,
            int_arr BIGINT[],
            float_arr FLOAT8[],
            bool_arr BOOLEAN[]
        );",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO array_test (int_arr, float_arr, bool_arr) VALUES (ARRAY[1,2,3], ARRAY[1.5,2.5], ARRAY[true,false]);"
    )
    .execute(&pool)
    .await
    .unwrap();

    let result = execute_page(&pool, "SELECT * FROM array_test ORDER BY id", 0)
        .await
        .unwrap();
    let row = &result.rows[0];
    let cols = &result.columns;

    let int_idx = cols.iter().position(|c| c == "int_arr").unwrap();
    assert_eq!(row[int_idx], "{1,2,3}");

    let float_idx = cols.iter().position(|c| c == "float_arr").unwrap();
    assert_eq!(row[float_idx], "{1.5,2.5}");

    let bool_idx = cols.iter().position(|c| c == "bool_arr").unwrap();
    assert_eq!(row[bool_idx], "{true,false}");
}

#[tokio::test]
async fn test_suggest_distinct_values() {
    let container = Postgres::default().start().await.unwrap();
    let host_ip = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(5432).await.unwrap();
    let connection_string = format!(
        "postgresql://postgres:postgres@{}:{}/postgres",
        host_ip, host_port
    );
    let pool = PgPool::connect(&connection_string).await.unwrap();

    sqlx::query("CREATE TABLE suggest_test (id SERIAL PRIMARY KEY, name TEXT);")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO suggest_test (name) VALUES ('Alice'), ('Alicia'), ('Bob'), ('Charlie');")
        .execute(&pool)
        .await
        .unwrap();

    let suggestions = sbql_core::query::suggest_distinct_values(
        &pool,
        "SELECT * FROM suggest_test",
        "name",
        "Al",
        10,
    )
    .await
    .unwrap();

    assert_eq!(suggestions.len(), 2);
    assert!(suggestions.contains(&"Alice".to_string()));
    assert!(suggestions.contains(&"Alicia".to_string()));
}

#[tokio::test]
async fn test_suggest_distinct_values_special_chars() {
    let container = Postgres::default().start().await.unwrap();
    let host_ip = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(5432).await.unwrap();
    let connection_string = format!(
        "postgresql://postgres:postgres@{}:{}/postgres",
        host_ip, host_port
    );
    let pool = PgPool::connect(&connection_string).await.unwrap();

    sqlx::query("CREATE TABLE special_test (id SERIAL PRIMARY KEY, val TEXT);")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO special_test (val) VALUES ('100% done'), ('50% complete'), ('hello');")
        .execute(&pool)
        .await
        .unwrap();

    // The % in prefix should be escaped
    let suggestions = sbql_core::query::suggest_distinct_values(
        &pool,
        "SELECT * FROM special_test",
        "val",
        "100%",
        10,
    )
    .await
    .unwrap();

    assert_eq!(suggestions.len(), 1);
    assert!(suggestions.contains(&"100% done".to_string()));
}

#[tokio::test]
async fn test_user_defined_limit_respected() {
    let container = Postgres::default().start().await.unwrap();
    let host_ip = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(5432).await.unwrap();
    let connection_string = format!(
        "postgresql://postgres:postgres@{}:{}/postgres",
        host_ip, host_port
    );
    let pool = PgPool::connect(&connection_string).await.unwrap();

    sqlx::query("CREATE TABLE limit_test (id SERIAL PRIMARY KEY);")
        .execute(&pool)
        .await
        .unwrap();

    for _ in 0..10 {
        sqlx::query("INSERT INTO limit_test DEFAULT VALUES")
            .execute(&pool)
            .await
            .unwrap();
    }

    // User's own LIMIT should be respected
    let result = execute_page(&pool, "SELECT * FROM limit_test LIMIT 3", 0)
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 3);
}
