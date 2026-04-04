use mongodb::bson::{doc, Document};
use sbql_core::{
    query::execute_page,
    schema::{get_primary_keys, list_tables, load_diagram},
    DbPool,
};
use testcontainers::{core::WaitFor, runners::AsyncRunner, GenericImage};

/// Start a MongoDB container and return the database handle + pool.
async fn setup_mongodb() -> (
    mongodb::Database,
    DbPool,
    testcontainers::ContainerAsync<GenericImage>,
) {
    let container = GenericImage::new("mongo", "7")
        .with_exposed_port(27017.into())
        .with_wait_for(WaitFor::message_on_stdout("Waiting for connections"))
        .start()
        .await
        .unwrap();

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(27017).await.unwrap();
    let uri = format!("mongodb://{}:{}", host, port);

    let client = mongodb::Client::with_uri_str(&uri).await.unwrap();
    let db = client.database("testdb");

    // Wait for MongoDB to be fully ready
    for _ in 0..10 {
        if db.run_command(doc! { "ping": 1 }).await.is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    let pool = DbPool::MongoDb(Box::new(db.clone()));
    (db, pool, container)
}

#[tokio::test]
async fn test_mongodb_list_tables() {
    let (db, pool, _container) = setup_mongodb().await;

    // Create two collections by inserting a document into each
    db.collection::<Document>("users")
        .insert_one(doc! { "name": "Alice" })
        .await
        .unwrap();
    db.collection::<Document>("orders")
        .insert_one(doc! { "item": "Widget" })
        .await
        .unwrap();

    let tables = list_tables(&pool).await.expect("list_tables failed");
    let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"users"), "Expected 'users', got: {names:?}");
    assert!(
        names.contains(&"orders"),
        "Expected 'orders', got: {names:?}"
    );
    assert_eq!(tables.len(), 2);
    for t in &tables {
        assert_eq!(t.schema, "mongodb");
    }
}

#[tokio::test]
async fn test_mongodb_get_primary_keys() {
    let (db, pool, _container) = setup_mongodb().await;

    // Create a collection
    db.collection::<Document>("items")
        .insert_one(doc! { "x": 1 })
        .await
        .unwrap();

    let pks = get_primary_keys(&pool, "mongodb", "items")
        .await
        .expect("get_primary_keys failed");
    assert_eq!(pks, vec!["_id"]);
}

#[tokio::test]
async fn test_mongodb_execute_page() {
    let (db, pool, _container) = setup_mongodb().await;

    // Insert 3 documents
    let coll = db.collection::<Document>("users");
    coll.insert_many(vec![
        doc! { "name": "Alice", "age": 30 },
        doc! { "name": "Bob", "age": 25 },
        doc! { "name": "Charlie", "age": 35 },
    ])
    .await
    .unwrap();

    // Pass collection name, not SQL
    let result = execute_page(&pool, "users", 0)
        .await
        .expect("execute_page failed");

    assert_eq!(result.rows.len(), 3, "Expected 3 rows");
    assert!(result.columns.contains(&"_id".to_string()));
    assert!(result.columns.contains(&"name".to_string()));
    assert!(result.columns.contains(&"age".to_string()));

    // Verify the values are present (order may vary)
    let name_idx = result.columns.iter().position(|c| c == "name").unwrap();
    let names: Vec<&str> = result.rows.iter().map(|r| r[name_idx].as_str()).collect();
    assert!(names.contains(&"Alice"));
    assert!(names.contains(&"Bob"));
    assert!(names.contains(&"Charlie"));
}

#[tokio::test]
async fn test_mongodb_attribute_types() {
    let (db, pool, _container) = setup_mongodb().await;

    let coll = db.collection::<Document>("typed");
    coll.insert_one(doc! {
        "str_val": "hello",
        "int32_val": 42_i32,
        "int64_val": 9_999_999_999_i64,
        "double_val": 3.14_f64,
        "bool_val": true,
        "oid_val": mongodb::bson::oid::ObjectId::new(),
        "array_val": [1_i32, 2_i32, 3_i32],
        "nested_val": { "inner_key": "inner_value" },
    })
    .await
    .unwrap();

    let result = execute_page(&pool, "typed", 0)
        .await
        .expect("execute_page failed");

    assert_eq!(result.rows.len(), 1);
    let cols = &result.columns;
    let row = &result.rows[0];

    // String
    let str_idx = cols.iter().position(|c| c == "str_val").unwrap();
    assert_eq!(row[str_idx], "hello");

    // Int32
    let i32_idx = cols.iter().position(|c| c == "int32_val").unwrap();
    assert_eq!(row[i32_idx], "42");

    // Int64
    let i64_idx = cols.iter().position(|c| c == "int64_val").unwrap();
    assert_eq!(row[i64_idx], "9999999999");

    // Double
    let dbl_idx = cols.iter().position(|c| c == "double_val").unwrap();
    assert!(
        row[dbl_idx].starts_with("3.14"),
        "double_val: {}",
        row[dbl_idx]
    );

    // Boolean
    let bool_idx = cols.iter().position(|c| c == "bool_val").unwrap();
    assert_eq!(row[bool_idx], "true");

    // ObjectId (24 hex chars)
    let oid_idx = cols.iter().position(|c| c == "oid_val").unwrap();
    assert_eq!(row[oid_idx].len(), 24, "oid_val: {}", row[oid_idx]);

    // Array
    let arr_idx = cols.iter().position(|c| c == "array_val").unwrap();
    assert!(row[arr_idx].contains("1"), "array_val: {}", row[arr_idx]);
    assert!(row[arr_idx].contains("2"), "array_val: {}", row[arr_idx]);
    assert!(row[arr_idx].contains("3"), "array_val: {}", row[arr_idx]);

    // Nested document
    let nested_idx = cols.iter().position(|c| c == "nested_val").unwrap();
    assert!(
        row[nested_idx].contains("inner_key"),
        "nested_val: {}",
        row[nested_idx]
    );
    assert!(
        row[nested_idx].contains("inner_value"),
        "nested_val: {}",
        row[nested_idx]
    );
}

#[tokio::test]
async fn test_mongodb_empty_collection() {
    let (db, pool, _container) = setup_mongodb().await;

    // Create an empty collection explicitly
    db.create_collection("empty_coll").await.unwrap();

    let result = execute_page(&pool, "empty_coll", 0)
        .await
        .expect("execute_page failed");

    assert_eq!(result.rows.len(), 0);
    assert!(result.columns.is_empty());
}

#[tokio::test]
async fn test_mongodb_diagram_empty() {
    let (_db, pool, _container) = setup_mongodb().await;

    let diagram = load_diagram(&pool).await.expect("load_diagram failed");
    // MongoDB has no FK relationships
    assert!(diagram.tables.is_empty());
    assert!(diagram.foreign_keys.is_empty());
}
