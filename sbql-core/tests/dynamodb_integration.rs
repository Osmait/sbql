use sbql_core::{
    query::execute_page,
    schema::{get_primary_keys, list_tables, load_diagram},
    DbPool,
};
use testcontainers::{core::WaitFor, runners::AsyncRunner, GenericImage};

/// Start a DynamoDB Local container and return the client + pool.
async fn setup_dynamodb() -> (
    aws_sdk_dynamodb::Client,
    DbPool,
    testcontainers::ContainerAsync<GenericImage>,
) {
    let container = GenericImage::new("amazon/dynamodb-local", "latest")
        .with_exposed_port(8000.into())
        .with_wait_for(WaitFor::message_on_stdout("Initializing DynamoDB Local"))
        .start()
        .await
        .unwrap();

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(8000).await.unwrap();
    let endpoint = format!("http://{}:{}", host, port);

    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::Region::new("us-east-1"))
        .credentials_provider(aws_sdk_dynamodb::config::Credentials::new(
            "fakekey",
            "fakesecret",
            None,
            None,
            "test",
        ))
        .load()
        .await;

    let dynamo_config = aws_sdk_dynamodb::config::Builder::from(&config)
        .endpoint_url(&endpoint)
        .build();

    let client = aws_sdk_dynamodb::Client::from_conf(dynamo_config);

    // Wait for DynamoDB to be fully ready (retry ListTables)
    for _ in 0..10 {
        if client.list_tables().limit(1).send().await.is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    let pool = DbPool::DynamoDb(Box::new(client.clone()));

    // Create test table: users (pk=user_id:S)
    client
        .create_table()
        .table_name("users")
        .key_schema(
            aws_sdk_dynamodb::types::KeySchemaElement::builder()
                .attribute_name("user_id")
                .key_type(aws_sdk_dynamodb::types::KeyType::Hash)
                .build()
                .unwrap(),
        )
        .attribute_definitions(
            aws_sdk_dynamodb::types::AttributeDefinition::builder()
                .attribute_name("user_id")
                .attribute_type(aws_sdk_dynamodb::types::ScalarAttributeType::S)
                .build()
                .unwrap(),
        )
        .billing_mode(aws_sdk_dynamodb::types::BillingMode::PayPerRequest)
        .send()
        .await
        .expect("Failed to create users table");

    // Create test table: posts (pk=post_id:S, sk=user_id:S)
    client
        .create_table()
        .table_name("posts")
        .key_schema(
            aws_sdk_dynamodb::types::KeySchemaElement::builder()
                .attribute_name("post_id")
                .key_type(aws_sdk_dynamodb::types::KeyType::Hash)
                .build()
                .unwrap(),
        )
        .key_schema(
            aws_sdk_dynamodb::types::KeySchemaElement::builder()
                .attribute_name("user_id")
                .key_type(aws_sdk_dynamodb::types::KeyType::Range)
                .build()
                .unwrap(),
        )
        .attribute_definitions(
            aws_sdk_dynamodb::types::AttributeDefinition::builder()
                .attribute_name("post_id")
                .attribute_type(aws_sdk_dynamodb::types::ScalarAttributeType::S)
                .build()
                .unwrap(),
        )
        .attribute_definitions(
            aws_sdk_dynamodb::types::AttributeDefinition::builder()
                .attribute_name("user_id")
                .attribute_type(aws_sdk_dynamodb::types::ScalarAttributeType::S)
                .build()
                .unwrap(),
        )
        .billing_mode(aws_sdk_dynamodb::types::BillingMode::PayPerRequest)
        .send()
        .await
        .expect("Failed to create posts table");

    // Seed users
    use aws_sdk_dynamodb::types::AttributeValue;
    for (id, name, age) in [("u1", "Alice", "30"), ("u2", "Bob", "25"), ("u3", "Charlie", "35")] {
        client
            .put_item()
            .table_name("users")
            .item("user_id", AttributeValue::S(id.into()))
            .item("name", AttributeValue::S(name.into()))
            .item("age", AttributeValue::N(age.into()))
            .send()
            .await
            .unwrap();
    }

    // Seed posts
    for (pid, uid, title) in [
        ("p1", "u1", "Hello World"),
        ("p2", "u1", "Second Post"),
        ("p3", "u2", "Bob's Post"),
    ] {
        client
            .put_item()
            .table_name("posts")
            .item("post_id", AttributeValue::S(pid.into()))
            .item("user_id", AttributeValue::S(uid.into()))
            .item("title", AttributeValue::S(title.into()))
            .send()
            .await
            .unwrap();
    }

    (client, pool, container)
}

#[tokio::test]
async fn test_dynamodb_list_tables() {
    let (_client, pool, _container) = setup_dynamodb().await;
    let tables = list_tables(&pool).await.expect("list_tables failed");
    let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"users"), "Expected 'users', got: {names:?}");
    assert!(names.contains(&"posts"), "Expected 'posts', got: {names:?}");
    assert_eq!(tables.len(), 2);
    for t in &tables {
        assert_eq!(t.schema, "dynamodb");
    }
}

#[tokio::test]
async fn test_dynamodb_get_primary_keys() {
    let (_client, pool, _container) = setup_dynamodb().await;

    // users has a single hash key
    let pks = get_primary_keys(&pool, "dynamodb", "users")
        .await
        .expect("get_primary_keys failed");
    assert_eq!(pks, vec!["user_id"]);

    // posts has hash + range key
    let pks = get_primary_keys(&pool, "dynamodb", "posts")
        .await
        .expect("get_primary_keys failed");
    assert_eq!(pks.len(), 2);
    assert!(pks.contains(&"post_id".to_string()));
    assert!(pks.contains(&"user_id".to_string()));
}

#[tokio::test]
async fn test_dynamodb_execute_partiql() {
    let (_client, pool, _container) = setup_dynamodb().await;

    let result = execute_page(&pool, "SELECT * FROM users", 0)
        .await
        .expect("execute_page failed");

    assert_eq!(result.rows.len(), 3, "Expected 3 users");
    assert!(result.columns.contains(&"user_id".to_string()));
    assert!(result.columns.contains(&"name".to_string()));
    assert!(result.columns.contains(&"age".to_string()));
}

#[tokio::test]
async fn test_dynamodb_partiql_with_where() {
    let (_client, pool, _container) = setup_dynamodb().await;

    let result = execute_page(&pool, "SELECT * FROM users WHERE user_id = 'u1'", 0)
        .await
        .expect("execute_page with WHERE failed");

    assert_eq!(result.rows.len(), 1);
    let name_idx = result.columns.iter().position(|c| c == "name").unwrap();
    assert_eq!(result.rows[0][name_idx], "Alice");
}

#[tokio::test]
async fn test_dynamodb_diagram_empty() {
    let (_client, pool, _container) = setup_dynamodb().await;
    let diagram = load_diagram(&pool).await.expect("load_diagram failed");
    // DynamoDB has no FK relationships
    assert!(diagram.tables.is_empty());
    assert!(diagram.foreign_keys.is_empty());
}

#[tokio::test]
async fn test_dynamodb_empty_table() {
    let container = GenericImage::new("amazon/dynamodb-local", "latest")
        .with_exposed_port(8000.into())
        .with_wait_for(WaitFor::message_on_stdout("Initializing DynamoDB Local"))
        .start()
        .await
        .unwrap();

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(8000).await.unwrap();
    let endpoint = format!("http://{}:{}", host, port);

    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::Region::new("us-east-1"))
        .credentials_provider(aws_sdk_dynamodb::config::Credentials::new(
            "fakekey", "fakesecret", None, None, "test",
        ))
        .load()
        .await;
    let dynamo_config = aws_sdk_dynamodb::config::Builder::from(&config)
        .endpoint_url(&endpoint)
        .build();
    let client = aws_sdk_dynamodb::Client::from_conf(dynamo_config);

    // Wait for DynamoDB to be fully ready
    for _ in 0..10 {
        if client.list_tables().limit(1).send().await.is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    client
        .create_table()
        .table_name("empty_table")
        .key_schema(
            aws_sdk_dynamodb::types::KeySchemaElement::builder()
                .attribute_name("id")
                .key_type(aws_sdk_dynamodb::types::KeyType::Hash)
                .build()
                .unwrap(),
        )
        .attribute_definitions(
            aws_sdk_dynamodb::types::AttributeDefinition::builder()
                .attribute_name("id")
                .attribute_type(aws_sdk_dynamodb::types::ScalarAttributeType::S)
                .build()
                .unwrap(),
        )
        .billing_mode(aws_sdk_dynamodb::types::BillingMode::PayPerRequest)
        .send()
        .await
        .unwrap();

    let pool = DbPool::DynamoDb(Box::new(client));
    let result = execute_page(&pool, "SELECT * FROM empty_table", 0)
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 0);
    assert!(result.columns.is_empty());
}

#[tokio::test]
async fn test_dynamodb_attribute_types() {
    let (_client, pool, _container) = setup_dynamodb().await;

    // Insert an item with various attribute types
    use aws_sdk_dynamodb::types::AttributeValue;
    let client = match &pool {
        DbPool::DynamoDb(c) => c,
        _ => panic!("Expected DynamoDb pool"),
    };

    client
        .put_item()
        .table_name("users")
        .item("user_id", AttributeValue::S("u_types".into()))
        .item("name", AttributeValue::S("TypeTest".into()))
        .item("age", AttributeValue::N("99".into()))
        .item("active", AttributeValue::Bool(true))
        .item("tags", AttributeValue::Ss(vec!["rust".into(), "aws".into()]))
        .item(
            "scores",
            AttributeValue::L(vec![
                AttributeValue::N("100".into()),
                AttributeValue::N("200".into()),
            ]),
        )
        .send()
        .await
        .unwrap();

    let result = execute_page(&pool, "SELECT * FROM users WHERE user_id = 'u_types'", 0)
        .await
        .unwrap();

    assert_eq!(result.rows.len(), 1);
    let cols = &result.columns;
    let row = &result.rows[0];

    // Check string
    let name_idx = cols.iter().position(|c| c == "name").unwrap();
    assert_eq!(row[name_idx], "TypeTest");

    // Check number
    let age_idx = cols.iter().position(|c| c == "age").unwrap();
    assert_eq!(row[age_idx], "99");

    // Check bool
    let active_idx = cols.iter().position(|c| c == "active").unwrap();
    assert_eq!(row[active_idx], "true");

    // Check string set
    let tags_idx = cols.iter().position(|c| c == "tags").unwrap();
    assert!(row[tags_idx].contains("rust"));
    assert!(row[tags_idx].contains("aws"));

    // Check list
    let scores_idx = cols.iter().position(|c| c == "scores").unwrap();
    assert!(row[scores_idx].contains("100"));
    assert!(row[scores_idx].contains("200"));
}

#[tokio::test]
async fn test_dynamodb_number_set() {
    let (_client, pool, _container) = setup_dynamodb().await;

    use aws_sdk_dynamodb::types::AttributeValue;
    let client = match &pool {
        DbPool::DynamoDb(c) => c,
        _ => panic!("Expected DynamoDb pool"),
    };

    client
        .put_item()
        .table_name("users")
        .item("user_id", AttributeValue::S("u_ns".into()))
        .item(
            "lucky_numbers",
            AttributeValue::Ns(vec!["7".into(), "13".into(), "42".into()]),
        )
        .send()
        .await
        .unwrap();

    let result = execute_page(&pool, "SELECT * FROM users WHERE user_id = 'u_ns'", 0)
        .await
        .unwrap();

    assert_eq!(result.rows.len(), 1);
    let cols = &result.columns;
    let row = &result.rows[0];
    let ns_idx = cols.iter().position(|c| c == "lucky_numbers").unwrap();
    assert!(row[ns_idx].contains("7"), "NS: {}", row[ns_idx]);
    assert!(row[ns_idx].contains("13"), "NS: {}", row[ns_idx]);
    assert!(row[ns_idx].contains("42"), "NS: {}", row[ns_idx]);
}

#[tokio::test]
async fn test_dynamodb_nested_map() {
    let (_client, pool, _container) = setup_dynamodb().await;

    use aws_sdk_dynamodb::types::AttributeValue;
    use std::collections::HashMap;

    let client = match &pool {
        DbPool::DynamoDb(c) => c,
        _ => panic!("Expected DynamoDb pool"),
    };

    // Build deeply nested map: {b: {c: {d: "deep"}}}
    let inner_most: HashMap<String, AttributeValue> =
        [("d".to_string(), AttributeValue::S("deep".into()))]
            .into_iter()
            .collect();
    let level_c: HashMap<String, AttributeValue> =
        [("c".to_string(), AttributeValue::M(inner_most))]
            .into_iter()
            .collect();
    let level_b: HashMap<String, AttributeValue> =
        [("b".to_string(), AttributeValue::M(level_c))]
            .into_iter()
            .collect();

    client
        .put_item()
        .table_name("users")
        .item("user_id", AttributeValue::S("u_nested".into()))
        .item("data", AttributeValue::M(level_b))
        .send()
        .await
        .unwrap();

    let result = execute_page(
        &pool,
        "SELECT * FROM users WHERE user_id = 'u_nested'",
        0,
    )
    .await
    .unwrap();

    assert_eq!(result.rows.len(), 1);
    let cols = &result.columns;
    let row = &result.rows[0];
    let data_idx = cols.iter().position(|c| c == "data").unwrap();
    assert!(
        row[data_idx].contains("deep"),
        "Expected nested 'deep' in: {}",
        row[data_idx]
    );
}

#[tokio::test]
async fn test_dynamodb_binary_type() {
    let (_client, pool, _container) = setup_dynamodb().await;

    use aws_sdk_dynamodb::types::AttributeValue;

    let client = match &pool {
        DbPool::DynamoDb(c) => c,
        _ => panic!("Expected DynamoDb pool"),
    };

    let binary_data = aws_sdk_dynamodb::primitives::Blob::new(vec![0xDE, 0xAD, 0xBE, 0xEF]);
    client
        .put_item()
        .table_name("users")
        .item("user_id", AttributeValue::S("u_bin".into()))
        .item("payload", AttributeValue::B(binary_data))
        .send()
        .await
        .unwrap();

    let result = execute_page(&pool, "SELECT * FROM users WHERE user_id = 'u_bin'", 0)
        .await
        .unwrap();

    assert_eq!(result.rows.len(), 1);
    let cols = &result.columns;
    let row = &result.rows[0];
    let bin_idx = cols.iter().position(|c| c == "payload").unwrap();
    // Binary is hex-encoded with \x prefix
    assert!(
        row[bin_idx].contains("deadbeef"),
        "Expected hex 'deadbeef' in: {}",
        row[bin_idx]
    );
}

#[tokio::test]
async fn test_dynamodb_multiple_tables() {
    let container = GenericImage::new("amazon/dynamodb-local", "latest")
        .with_exposed_port(8000.into())
        .with_wait_for(WaitFor::message_on_stdout("Initializing DynamoDB Local"))
        .start()
        .await
        .unwrap();

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(8000).await.unwrap();
    let endpoint = format!("http://{}:{}", host, port);

    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::Region::new("us-east-1"))
        .credentials_provider(aws_sdk_dynamodb::config::Credentials::new(
            "fakekey", "fakesecret", None, None, "test",
        ))
        .load()
        .await;
    let dynamo_config = aws_sdk_dynamodb::config::Builder::from(&config)
        .endpoint_url(&endpoint)
        .build();
    let client = aws_sdk_dynamodb::Client::from_conf(dynamo_config);

    for _ in 0..10 {
        if client.list_tables().limit(1).send().await.is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    // Create 3 tables
    for table_name in ["alpha", "beta", "gamma"] {
        client
            .create_table()
            .table_name(table_name)
            .key_schema(
                aws_sdk_dynamodb::types::KeySchemaElement::builder()
                    .attribute_name("id")
                    .key_type(aws_sdk_dynamodb::types::KeyType::Hash)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                aws_sdk_dynamodb::types::AttributeDefinition::builder()
                    .attribute_name("id")
                    .attribute_type(aws_sdk_dynamodb::types::ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .billing_mode(aws_sdk_dynamodb::types::BillingMode::PayPerRequest)
            .send()
            .await
            .unwrap();
    }

    let pool = DbPool::DynamoDb(Box::new(client));
    let tables = list_tables(&pool).await.expect("list_tables failed");
    let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"alpha"), "Missing alpha: {names:?}");
    assert!(names.contains(&"beta"), "Missing beta: {names:?}");
    assert!(names.contains(&"gamma"), "Missing gamma: {names:?}");
    assert_eq!(tables.len(), 3);
}

#[tokio::test]
async fn test_dynamodb_null_attribute() {
    let (_client, pool, _container) = setup_dynamodb().await;

    use aws_sdk_dynamodb::types::AttributeValue;
    let client = match &pool {
        DbPool::DynamoDb(c) => c,
        _ => panic!("Expected DynamoDb pool"),
    };

    client
        .put_item()
        .table_name("users")
        .item("user_id", AttributeValue::S("u_null".into()))
        .item("name", AttributeValue::S("NullTest".into()))
        .item("optional", AttributeValue::Null(true))
        .send()
        .await
        .unwrap();

    let result = execute_page(
        &pool,
        "SELECT * FROM users WHERE user_id = 'u_null'",
        0,
    )
    .await
    .unwrap();

    assert_eq!(result.rows.len(), 1);
    let cols = &result.columns;
    let row = &result.rows[0];
    let null_idx = cols.iter().position(|c| c == "optional").unwrap();
    assert_eq!(row[null_idx], "", "Null attribute should be empty string");
}
