use sbql_core::{
    query::execute_page,
    schema::{list_tables, load_diagram},
    DbPool,
};
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::redis::Redis;

async fn setup_redis() -> (DbPool, testcontainers::ContainerAsync<Redis>) {
    let container = Redis::default().start().await.unwrap();
    let host_ip = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(6379).await.unwrap();

    let url = format!("redis://{}:{}", host_ip, host_port);
    let client = redis::Client::open(url.as_str()).expect("Failed to create Redis client");
    let cm = redis::aio::ConnectionManager::new(client)
        .await
        .expect("Failed to create ConnectionManager");
    let pool = DbPool::Redis(Box::new(cm));

    (pool, container)
}

#[tokio::test]
async fn test_redis_connect_and_ping() {
    let container = Redis::default().start().await.unwrap();
    let host_ip = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(6379).await.unwrap();

    let url = format!("redis://{}:{}", host_ip, host_port);
    let client = redis::Client::open(url.as_str()).unwrap();
    let mut cm = redis::aio::ConnectionManager::new(client).await.unwrap();

    let pong: String = redis::cmd("PING").query_async(&mut cm).await.unwrap();
    assert_eq!(pong, "PONG");
}

#[tokio::test]
async fn test_redis_set_and_get() {
    let (pool, _container) = setup_redis().await;

    // SET key value
    let result = execute_page(&pool, "SET mykey myvalue", 0).await.unwrap();
    assert_eq!(result.columns, vec!["value"]);
    assert_eq!(result.rows[0][0], "OK");

    // GET key
    let result = execute_page(&pool, "GET mykey", 0).await.unwrap();
    assert_eq!(result.columns, vec!["value"]);
    assert_eq!(result.rows[0][0], "myvalue");
    assert!(!result.has_next_page);
}

#[tokio::test]
async fn test_redis_get_missing_key() {
    let (pool, _container) = setup_redis().await;

    let result = execute_page(&pool, "GET nonexistent", 0).await.unwrap();
    assert_eq!(result.columns, vec!["value"]);
    assert_eq!(result.rows[0][0], "(nil)");
}

#[tokio::test]
async fn test_redis_keys_command() {
    let (pool, _container) = setup_redis().await;

    // SET multiple keys
    execute_page(&pool, "SET key1 val1", 0).await.unwrap();
    execute_page(&pool, "SET key2 val2", 0).await.unwrap();
    execute_page(&pool, "SET key3 val3", 0).await.unwrap();

    // KEYS *
    let result = execute_page(&pool, "KEYS *", 0).await.unwrap();
    // Should return an array with index/value columns
    assert!(result.columns.contains(&"value".to_string()));
    assert_eq!(result.rows.len(), 3);
}

#[tokio::test]
async fn test_redis_hash_commands() {
    let (pool, _container) = setup_redis().await;

    // HSET hash field value
    execute_page(&pool, "HSET myhash field1 value1", 0)
        .await
        .unwrap();
    execute_page(&pool, "HSET myhash field2 value2", 0)
        .await
        .unwrap();

    // HGETALL
    let result = execute_page(&pool, "HGETALL myhash", 0).await.unwrap();
    assert_eq!(result.columns, vec!["field", "value"]);
    assert_eq!(result.rows.len(), 2);

    // Verify field/value pairs exist (order may vary)
    let has_field1 = result
        .rows
        .iter()
        .any(|r| r[0] == "field1" && r[1] == "value1");
    let has_field2 = result
        .rows
        .iter()
        .any(|r| r[0] == "field2" && r[1] == "value2");
    assert!(has_field1);
    assert!(has_field2);
}

#[tokio::test]
async fn test_redis_list_commands() {
    let (pool, _container) = setup_redis().await;

    // LPUSH list items
    execute_page(&pool, "LPUSH mylist a", 0).await.unwrap();
    execute_page(&pool, "LPUSH mylist b", 0).await.unwrap();
    execute_page(&pool, "LPUSH mylist c", 0).await.unwrap();

    // LRANGE list 0 -1
    let result = execute_page(&pool, "LRANGE mylist 0 -1", 0).await.unwrap();
    assert_eq!(result.columns, vec!["index", "value"]);
    assert_eq!(result.rows.len(), 3);
    // LPUSH pushes to head, so order is c, b, a
    assert_eq!(result.rows[0][1], "c");
    assert_eq!(result.rows[1][1], "b");
    assert_eq!(result.rows[2][1], "a");
}

#[tokio::test]
async fn test_redis_del_command() {
    let (pool, _container) = setup_redis().await;

    execute_page(&pool, "SET delme hello", 0).await.unwrap();

    let result = execute_page(&pool, "DEL delme", 0).await.unwrap();
    assert_eq!(result.columns, vec!["value"]);
    assert_eq!(result.rows[0][0], "1"); // 1 key deleted
}

#[tokio::test]
async fn test_redis_info_command() {
    let (pool, _container) = setup_redis().await;

    let result = execute_page(&pool, "INFO server", 0).await.unwrap();
    // INFO returns a bulk string; should have at least one row
    assert!(!result.rows.is_empty());
}

#[tokio::test]
async fn test_redis_list_tables_empty() {
    let (pool, _container) = setup_redis().await;

    let tables = list_tables(&pool).await.unwrap();
    assert!(tables.is_empty());
}

#[tokio::test]
async fn test_redis_diagram_empty() {
    let (pool, _container) = setup_redis().await;

    let diagram = load_diagram(&pool).await.unwrap();
    assert!(diagram.tables.is_empty());
    assert!(diagram.foreign_keys.is_empty());
}

#[tokio::test]
async fn test_redis_mset_mget() {
    let (pool, _container) = setup_redis().await;

    // MSET multiple keys at once
    let result = execute_page(&pool, "MSET k1 v1 k2 v2 k3 v3", 0)
        .await
        .unwrap();
    assert_eq!(result.rows[0][0], "OK");

    // MGET all three keys
    let result = execute_page(&pool, "MGET k1 k2 k3", 0).await.unwrap();
    assert_eq!(result.rows.len(), 3);

    let values: Vec<&str> = result.rows.iter().map(|r| r.last().unwrap().as_str()).collect();
    assert!(values.contains(&"v1"), "Missing v1: {values:?}");
    assert!(values.contains(&"v2"), "Missing v2: {values:?}");
    assert!(values.contains(&"v3"), "Missing v3: {values:?}");
}

#[tokio::test]
async fn test_redis_expire_and_ttl() {
    let (pool, _container) = setup_redis().await;

    // SET with EX (expire in seconds)
    execute_page(&pool, "SET ttlkey somevalue EX 300", 0)
        .await
        .unwrap();

    // TTL should return a positive number
    let result = execute_page(&pool, "TTL ttlkey", 0).await.unwrap();
    let ttl: i64 = result.rows[0][0].parse().expect("TTL should be a number");
    assert!(ttl > 0, "Expected positive TTL, got: {ttl}");
    assert!(ttl <= 300, "TTL should be <= 300, got: {ttl}");
}

#[tokio::test]
async fn test_redis_incr_decr() {
    let (pool, _container) = setup_redis().await;

    execute_page(&pool, "SET counter 10", 0).await.unwrap();

    let result = execute_page(&pool, "INCR counter", 0).await.unwrap();
    assert_eq!(result.rows[0][0], "11");

    let result = execute_page(&pool, "INCR counter", 0).await.unwrap();
    assert_eq!(result.rows[0][0], "12");

    let result = execute_page(&pool, "DECR counter", 0).await.unwrap();
    assert_eq!(result.rows[0][0], "11");
}

#[tokio::test]
async fn test_redis_large_value() {
    let (pool, _container) = setup_redis().await;

    // Create a 10KB value
    let large_value: String = "A".repeat(10 * 1024);
    let cmd = format!("SET bigkey {}", large_value);
    execute_page(&pool, &cmd, 0).await.unwrap();

    let result = execute_page(&pool, "GET bigkey", 0).await.unwrap();
    assert_eq!(result.rows[0][0].len(), 10 * 1024);
    assert_eq!(result.rows[0][0], large_value);
}

#[tokio::test]
async fn test_redis_unicode_values() {
    let (pool, _container) = setup_redis().await;

    // SET emoji value
    execute_page(&pool, "SET emoji \u{1F600}\u{1F680}", 0)
        .await
        .unwrap();
    let result = execute_page(&pool, "GET emoji", 0).await.unwrap();
    assert_eq!(result.rows[0][0], "\u{1F600}\u{1F680}");

    // SET CJK value
    execute_page(&pool, "SET cjk \u{4F60}\u{597D}\u{4E16}\u{754C}", 0)
        .await
        .unwrap();
    let result = execute_page(&pool, "GET cjk", 0).await.unwrap();
    assert_eq!(result.rows[0][0], "\u{4F60}\u{597D}\u{4E16}\u{754C}");
}

#[tokio::test]
async fn test_redis_sorted_set() {
    let (pool, _container) = setup_redis().await;

    // ZADD members with scores
    execute_page(&pool, "ZADD leaderboard 100 alice", 0)
        .await
        .unwrap();
    execute_page(&pool, "ZADD leaderboard 200 bob", 0)
        .await
        .unwrap();
    execute_page(&pool, "ZADD leaderboard 150 charlie", 0)
        .await
        .unwrap();

    // ZRANGE returns members in score order
    let result = execute_page(&pool, "ZRANGE leaderboard 0 -1", 0)
        .await
        .unwrap();

    assert_eq!(result.rows.len(), 3);
    // Sorted by score ascending: alice(100), charlie(150), bob(200)
    let values: Vec<&str> = result.rows.iter().map(|r| r.last().unwrap().as_str()).collect();
    assert_eq!(values, vec!["alice", "charlie", "bob"]);
}
