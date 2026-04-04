//! PostgreSQL integration benchmarks using testcontainers.
//!
//! Requires Docker running. These benchmarks measure real PG wire-protocol
//! round-trips including pg_value_to_string type conversion.
//!
//! Run with: cargo bench --package sbql-core --bench postgres_integration

use criterion::{criterion_group, criterion_main, Criterion};
use sqlx::PgPool;
use tokio::runtime::Runtime;

use sbql_core::pool::DbPool;
use sbql_core::query::{execute_page, suggest_distinct_values};
use sbql_core::query_builder::{apply_filter, apply_order, clear_order, SortDirection};
use sbql_core::schema::{get_primary_keys, list_tables, load_diagram};
use sbql_core::DbBackend;

use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

/// Spin up a PG container and seed test data.
async fn setup_pg(
    user_count: usize,
    post_count: usize,
) -> (DbPool, PgPool, testcontainers::ContainerAsync<Postgres>) {
    let container = Postgres::default().start().await.unwrap();
    let host_ip = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(5432).await.unwrap();
    let url = format!(
        "postgresql://postgres:postgres@{}:{}/postgres",
        host_ip, host_port
    );
    let pg_pool = PgPool::connect(&url).await.unwrap();

    sqlx::query(
        "CREATE TABLE users (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            active BOOLEAN NOT NULL DEFAULT true,
            city TEXT,
            score NUMERIC(10,2),
            bio TEXT,
            tags TEXT[],
            created_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )",
    )
    .execute(&pg_pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE posts (
            id SERIAL PRIMARY KEY,
            user_id INTEGER NOT NULL REFERENCES users(id),
            title TEXT NOT NULL,
            body TEXT,
            metadata JSONB,
            created_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )",
    )
    .execute(&pg_pool)
    .await
    .unwrap();

    let cities = ["New York", "London", "Tokyo", "Berlin", "Paris"];
    for i in 0..user_count {
        sqlx::query(
            "INSERT INTO users (name, email, active, city, score, bio, tags)
             VALUES ($1, $2, $3, $4, $5::numeric, $6, $7)",
        )
        .bind(format!("user_{i}"))
        .bind(format!("user_{i}@example.com"))
        .bind(i % 5 != 0)
        .bind(cities[i % 5])
        .bind(format!("{}.{:02}", i * 10, i % 100))
        .bind(format!("Bio text for user {i} with some content."))
        .bind(vec![format!("tag_{}", i % 10), format!("tag_{}", i % 3)])
        .execute(&pg_pool)
        .await
        .unwrap();
    }

    for i in 0..post_count {
        let user_id = (i % user_count) + 1;
        sqlx::query(
            "INSERT INTO posts (user_id, title, body, metadata)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(user_id as i32)
        .bind(format!("Post title {i}"))
        .bind(format!("Body content for post {i} with extra text."))
        .bind(serde_json::json!({"views": i * 10, "category": format!("cat_{}", i % 5)}))
        .execute(&pg_pool)
        .await
        .unwrap();
    }

    let pool = DbPool::Postgres(pg_pool.clone());
    (pool, pg_pool, container)
}

fn bench_postgres(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (pool, _pg, _container) = rt.block_on(setup_pg(200, 500));

    // --- execute_page: tests pg_value_to_string hot path ---
    {
        let mut group = c.benchmark_group("pg_execute_page");
        group.sample_size(50);

        group.bench_function("simple_select", |b| {
            b.to_async(&rt).iter(|| async {
                execute_page(&pool, "SELECT * FROM users", 0)
                    .await
                    .unwrap()
            })
        });

        group.bench_function("with_types", |b| {
            b.to_async(&rt).iter(|| async {
                execute_page(
                    &pool,
                    "SELECT id, name, email, active, score, tags, created_at FROM users",
                    0,
                )
                .await
                .unwrap()
            })
        });

        group.bench_function("join", |b| {
            b.to_async(&rt).iter(|| async {
                execute_page(
                    &pool,
                    "SELECT p.id, p.title, p.metadata, u.name, u.tags
                     FROM posts p JOIN users u ON p.user_id = u.id",
                    0,
                )
                .await
                .unwrap()
            })
        });

        group.bench_function("filtered", |b| {
            b.to_async(&rt).iter(|| async {
                execute_page(&pool, "SELECT * FROM users WHERE active = true", 0)
                    .await
                    .unwrap()
            })
        });

        group.bench_function("page_1", |b| {
            b.to_async(&rt).iter(|| async {
                execute_page(&pool, "SELECT * FROM users", 1)
                    .await
                    .unwrap()
            })
        });

        group.finish();
    }

    // --- full lifecycle: order → filter → execute ---
    {
        let mut group = c.benchmark_group("pg_full_lifecycle");
        group.sample_size(50);

        group.bench_function("order_filter_execute", |b| {
            b.to_async(&rt).iter(|| async {
                let base = "SELECT * FROM users";
                let ordered =
                    apply_order(base, "name", SortDirection::Ascending, DbBackend::Postgres)
                        .unwrap();
                let cols: Vec<String> = vec!["name".into(), "email".into(), "city".into()];
                let filtered =
                    apply_filter(&ordered, "user_1", Some(&cols), DbBackend::Postgres).unwrap();
                execute_page(&pool, &filtered, 0).await.unwrap()
            })
        });

        group.bench_function("clear_order_execute", |b| {
            b.to_async(&rt).iter(|| async {
                let ordered = apply_order(
                    "SELECT * FROM users",
                    "created_at",
                    SortDirection::Descending,
                    DbBackend::Postgres,
                )
                .unwrap();
                let cleared = clear_order(&ordered, DbBackend::Postgres).unwrap();
                execute_page(&pool, &cleared, 0).await.unwrap()
            })
        });

        group.finish();
    }

    // --- schema introspection ---
    {
        let mut group = c.benchmark_group("pg_schema");
        group.sample_size(50);

        group.bench_function("list_tables", |b| {
            b.to_async(&rt)
                .iter(|| async { list_tables(&pool).await.unwrap() })
        });

        group.bench_function("load_diagram", |b| {
            b.to_async(&rt)
                .iter(|| async { load_diagram(&pool).await.unwrap() })
        });

        group.bench_function("get_primary_keys", |b| {
            b.to_async(&rt)
                .iter(|| async { get_primary_keys(&pool, "public", "users").await.unwrap() })
        });

        group.finish();
    }

    // --- suggest_distinct_values ---
    {
        let mut group = c.benchmark_group("pg_suggest_distinct_values");
        group.sample_size(50);

        group.bench_function("prefix_match", |b| {
            b.to_async(&rt).iter(|| async {
                suggest_distinct_values(&pool, "SELECT * FROM users", "name", "user_1", 20)
                    .await
                    .unwrap()
            })
        });

        group.bench_function("empty_prefix", |b| {
            b.to_async(&rt).iter(|| async {
                suggest_distinct_values(&pool, "SELECT * FROM users", "city", "", 20)
                    .await
                    .unwrap()
            })
        });

        group.finish();
    }
}

criterion_group!(benches, bench_postgres);
criterion_main!(benches);
