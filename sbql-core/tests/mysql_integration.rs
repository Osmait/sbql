use sbql_core::{
    query::{execute_page, suggest_distinct_values},
    schema::{
        execute_cell_update, execute_row_delete, get_primary_keys, list_tables, load_diagram,
    },
    DbPool,
};
use sqlx::{MySqlPool, Row};
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::mysql::Mysql;

/// Helper: start a MySQL container and create test tables with seed data.
async fn setup_mysql() -> (MySqlPool, DbPool, testcontainers::ContainerAsync<Mysql>) {
    let container = Mysql::default().start().await.unwrap();
    let host_ip = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(3306).await.unwrap();

    // testcontainers-modules mysql defaults: user=root, password=<empty or "my-secret-pw">, db=test
    // Try the default connection
    let connection_string = format!("mysql://root@{}:{}/test", host_ip, host_port);
    let my_pool = MySqlPool::connect(&connection_string)
        .await
        .expect("Failed to connect to MySQL container");

    // Create test tables
    sqlx::query(
        "CREATE TABLE users (
            id INT AUTO_INCREMENT PRIMARY KEY,
            username VARCHAR(50) NOT NULL UNIQUE,
            active BOOLEAN DEFAULT TRUE
        )",
    )
    .execute(&my_pool)
    .await
    .expect("Failed to create users table");

    sqlx::query(
        "CREATE TABLE posts (
            id INT AUTO_INCREMENT PRIMARY KEY,
            user_id INT NOT NULL,
            title VARCHAR(100) NOT NULL,
            content TEXT,
            FOREIGN KEY (user_id) REFERENCES users(id)
        )",
    )
    .execute(&my_pool)
    .await
    .expect("Failed to create posts table");

    sqlx::query("INSERT INTO users (username) VALUES ('alice'), ('bob')")
        .execute(&my_pool)
        .await
        .expect("Failed to insert users");

    sqlx::query(
        "INSERT INTO posts (user_id, title, content) VALUES
        (1, 'Alice First Post', 'Hello world'),
        (2, 'Bob Post', 'Hi there')",
    )
    .execute(&my_pool)
    .await
    .expect("Failed to insert posts");

    let pool = DbPool::Mysql(my_pool.clone());
    (my_pool, pool, container)
}

#[tokio::test]
async fn test_mysql_list_tables() {
    let (_my, pool, _container) = setup_mysql().await;
    let tables = list_tables(&pool).await.expect("list_tables failed");
    let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"users"), "Expected 'users' table, got: {names:?}");
    assert!(names.contains(&"posts"), "Expected 'posts' table, got: {names:?}");
    for t in &tables {
        assert_eq!(t.schema, "test", "Schema should be 'test' (MySQL database name)");
    }
}

#[tokio::test]
async fn test_mysql_get_primary_keys() {
    let (_my, pool, _container) = setup_mysql().await;
    let pks = get_primary_keys(&pool, "test", "users")
        .await
        .expect("get_primary_keys failed");
    assert_eq!(pks, vec!["id".to_string()]);
}

#[tokio::test]
async fn test_mysql_execute_page() {
    let (_my, pool, _container) = setup_mysql().await;
    let result = execute_page(&pool, "SELECT * FROM users ORDER BY id", 0)
        .await
        .expect("execute_page failed");
    assert_eq!(result.rows.len(), 2);
    assert!(result.columns.contains(&"id".to_string()));
    assert!(result.columns.contains(&"username".to_string()));
    assert!(result.columns.contains(&"active".to_string()));
}

#[tokio::test]
async fn test_mysql_diagram() {
    let (_my, pool, _container) = setup_mysql().await;
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
async fn test_mysql_cell_update() {
    let (my, pool, _container) = setup_mysql().await;
    execute_cell_update(&pool, "test", "users", "id", "1", "username", "alice_updated")
        .await
        .expect("cell update failed");

    let updated: String = sqlx::query("SELECT username FROM users WHERE id = 1")
        .fetch_one(&my)
        .await
        .unwrap()
        .get("username");
    assert_eq!(updated, "alice_updated");
}

#[tokio::test]
async fn test_mysql_row_delete() {
    let (my, pool, _container) = setup_mysql().await;
    execute_row_delete(&pool, "test", "posts", "id", "2")
        .await
        .expect("row delete failed");

    let count: i64 = sqlx::query("SELECT COUNT(*) AS c FROM posts WHERE id = 2")
        .fetch_one(&my)
        .await
        .unwrap()
        .get("c");
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_mysql_pagination() {
    let container = Mysql::default().start().await.unwrap();
    let host_ip = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(3306).await.unwrap();
    let connection_string = format!("mysql://root@{}:{}/test", host_ip, host_port);
    let my_pool = MySqlPool::connect(&connection_string).await.unwrap();
    let pool = DbPool::Mysql(my_pool.clone());

    sqlx::query("CREATE TABLE many_rows (id INT AUTO_INCREMENT PRIMARY KEY, val VARCHAR(50))")
        .execute(&my_pool)
        .await
        .unwrap();

    for i in 0..150 {
        sqlx::query("INSERT INTO many_rows (val) VALUES (?)")
            .bind(format!("row_{i}"))
            .execute(&my_pool)
            .await
            .unwrap();
    }

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
async fn test_mysql_types() {
    let container = Mysql::default().start().await.unwrap();
    let host_ip = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(3306).await.unwrap();
    let connection_string = format!("mysql://root@{}:{}/test", host_ip, host_port);
    let my_pool = MySqlPool::connect(&connection_string).await.unwrap();
    let pool = DbPool::Mysql(my_pool.clone());

    sqlx::query(
        "CREATE TABLE type_test (
            id INT AUTO_INCREMENT PRIMARY KEY,
            bool_val BOOLEAN,
            small_val SMALLINT,
            int_val INT,
            big_val BIGINT,
            float_val FLOAT,
            double_val DOUBLE,
            decimal_val DECIMAL(10,2),
            text_val TEXT,
            date_val DATE,
            datetime_val DATETIME,
            timestamp_val TIMESTAMP NULL,
            time_val TIME,
            json_val JSON,
            blob_val BLOB,
            null_val TEXT
        )",
    )
    .execute(&my_pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO type_test (bool_val, small_val, int_val, big_val, float_val, double_val, decimal_val, text_val, date_val, datetime_val, timestamp_val, time_val, json_val, blob_val, null_val) VALUES (TRUE, 42, 1000, 9999999999, 3.14, 2.718281828, 123.45, 'hello', '2024-01-15', '2024-01-15 10:30:00', '2024-01-15 10:30:00', '10:30:00', '{\"key\": \"value\"}', X'DEADBEEF', NULL)"
    )
    .execute(&my_pool)
    .await
    .unwrap();

    let result = execute_page(&pool, "SELECT * FROM type_test", 0).await.unwrap();
    assert_eq!(result.rows.len(), 1);
    let row = &result.rows[0];
    let cols = &result.columns;

    let bool_idx = cols.iter().position(|c| c == "bool_val").unwrap();
    assert_eq!(row[bool_idx], "true");

    let int_idx = cols.iter().position(|c| c == "int_val").unwrap();
    assert_eq!(row[int_idx], "1000");

    let big_idx = cols.iter().position(|c| c == "big_val").unwrap();
    assert_eq!(row[big_idx], "9999999999");

    let text_idx = cols.iter().position(|c| c == "text_val").unwrap();
    assert_eq!(row[text_idx], "hello");

    let date_idx = cols.iter().position(|c| c == "date_val").unwrap();
    assert_eq!(row[date_idx], "2024-01-15");

    let datetime_idx = cols.iter().position(|c| c == "datetime_val").unwrap();
    assert!(row[datetime_idx].contains("2024-01-15"), "datetime: {}", row[datetime_idx]);

    let timestamp_idx = cols.iter().position(|c| c == "timestamp_val").unwrap();
    assert!(row[timestamp_idx].contains("2024-01-15"), "timestamp: {}", row[timestamp_idx]);

    let time_idx = cols.iter().position(|c| c == "time_val").unwrap();
    assert!(row[time_idx].contains("10:30:00"), "time: {}", row[time_idx]);

    let json_idx = cols.iter().position(|c| c == "json_val").unwrap();
    assert!(row[json_idx].contains("key"), "json: {}", row[json_idx]);

    let blob_idx = cols.iter().position(|c| c == "blob_val").unwrap();
    assert_eq!(row[blob_idx], "\\xdeadbeef");

    let null_idx = cols.iter().position(|c| c == "null_val").unwrap();
    assert_eq!(row[null_idx], "");
}

#[tokio::test]
async fn test_mysql_suggest_distinct_values() {
    let container = Mysql::default().start().await.unwrap();
    let host_ip = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(3306).await.unwrap();
    let connection_string = format!("mysql://root@{}:{}/test", host_ip, host_port);
    let my_pool = MySqlPool::connect(&connection_string).await.unwrap();
    let pool = DbPool::Mysql(my_pool.clone());

    sqlx::query("CREATE TABLE suggest_test (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(50))")
        .execute(&my_pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO suggest_test (name) VALUES ('Alice'), ('Alicia'), ('Bob'), ('Charlie')")
        .execute(&my_pool)
        .await
        .unwrap();

    let suggestions = suggest_distinct_values(&pool, "SELECT * FROM suggest_test", "name", "Al", 10)
        .await
        .unwrap();

    assert_eq!(suggestions.len(), 2);
    assert!(suggestions.contains(&"Alice".to_string()));
    assert!(suggestions.contains(&"Alicia".to_string()));
}

#[tokio::test]
async fn test_mysql_empty_result() {
    let container = Mysql::default().start().await.unwrap();
    let host_ip = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(3306).await.unwrap();
    let connection_string = format!("mysql://root@{}:{}/test", host_ip, host_port);
    let my_pool = MySqlPool::connect(&connection_string).await.unwrap();
    let pool = DbPool::Mysql(my_pool.clone());

    sqlx::query("CREATE TABLE empty_table (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(50))")
        .execute(&my_pool)
        .await
        .unwrap();

    let result = execute_page(&pool, "SELECT * FROM empty_table", 0).await.unwrap();
    assert_eq!(result.rows.len(), 0);
    assert!(!result.has_next_page);
    assert!(result.columns.is_empty());
}
