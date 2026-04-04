//! Async SQLite integration benchmarks for query execution, schema introspection,
//! and the full query lifecycle.

use criterion::{criterion_group, criterion_main, Criterion};
use sqlx::SqlitePool;
use tokio::runtime::Runtime;

use sbql_core::pool::DbPool;
use sbql_core::query::execute_page;
use sbql_core::query_builder::{apply_filter, apply_order, SortDirection};
use sbql_core::schema::{list_tables, load_diagram};
use sbql_core::DbBackend;

/// Create an in-memory SQLite database with test data.
async fn setup_db(user_count: usize, post_count: usize) -> DbPool {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("SQLite in-memory pool");

    sqlx::query(
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            active INTEGER NOT NULL DEFAULT 1,
            city TEXT
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE posts (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL REFERENCES users(id),
            title TEXT NOT NULL,
            body TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Seed users
    for i in 0..user_count {
        sqlx::query("INSERT INTO users (name, email, active, city) VALUES (?, ?, ?, ?)")
            .bind(format!("user_{i}"))
            .bind(format!("user_{i}@example.com"))
            .bind(if i % 5 == 0 { 0 } else { 1 })
            .bind(["New York", "London", "Tokyo", "Berlin", "Paris"][i % 5])
            .execute(&pool)
            .await
            .unwrap();
    }

    // Seed posts
    for i in 0..post_count {
        let user_id = (i % user_count) + 1;
        sqlx::query("INSERT INTO posts (user_id, title, body) VALUES (?, ?, ?)")
            .bind(user_id as i64)
            .bind(format!("Post title {i}"))
            .bind(format!("Body content for post {i} with some extra text to make it realistic."))
            .execute(&pool)
            .await
            .unwrap();
    }

    DbPool::Sqlite(pool)
}

// ---------------------------------------------------------------------------
// execute_page
// ---------------------------------------------------------------------------

fn bench_execute_page(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let pool = rt.block_on(setup_db(200, 500));

    let mut group = c.benchmark_group("execute_page");

    group.bench_function("simple_select_page0", |b| {
        b.to_async(&rt).iter(|| async {
            execute_page(&pool, "SELECT * FROM users", 0)
                .await
                .unwrap()
        })
    });

    group.bench_function("simple_select_page1", |b| {
        b.to_async(&rt).iter(|| async {
            execute_page(&pool, "SELECT * FROM users", 1)
                .await
                .unwrap()
        })
    });

    group.bench_function("filtered", |b| {
        b.to_async(&rt).iter(|| async {
            execute_page(&pool, "SELECT * FROM users WHERE active = 1", 0)
                .await
                .unwrap()
        })
    });

    group.bench_function("join", |b| {
        b.to_async(&rt).iter(|| async {
            execute_page(
                &pool,
                "SELECT p.id, p.title, u.name FROM posts p JOIN users u ON p.user_id = u.id",
                0,
            )
            .await
            .unwrap()
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// full_lifecycle: order → filter → execute
// ---------------------------------------------------------------------------

fn bench_full_lifecycle(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let pool = rt.block_on(setup_db(200, 500));

    let mut group = c.benchmark_group("full_lifecycle");

    group.bench_function("order_filter_execute", |b| {
        b.to_async(&rt).iter(|| async {
            let base = "SELECT * FROM users";
            let ordered =
                apply_order(base, "name", SortDirection::Ascending, DbBackend::Sqlite).unwrap();
            let cols: Vec<String> = vec!["name".into(), "email".into(), "city".into()];
            let filtered =
                apply_filter(&ordered, "user_1", Some(&cols), DbBackend::Sqlite).unwrap();
            execute_page(&pool, &filtered, 0).await.unwrap()
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// schema introspection
// ---------------------------------------------------------------------------

fn bench_schema(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let pool = rt.block_on(setup_db(200, 500));

    let mut group = c.benchmark_group("schema");

    group.bench_function("list_tables", |b| {
        b.to_async(&rt)
            .iter(|| async { list_tables(&pool).await.unwrap() })
    });

    group.bench_function("load_diagram", |b| {
        b.to_async(&rt)
            .iter(|| async { load_diagram(&pool).await.unwrap() })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// suggest_distinct_values
// ---------------------------------------------------------------------------

fn bench_suggest_distinct_values(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let pool = rt.block_on(setup_db(200, 500));

    let mut group = c.benchmark_group("suggest_distinct_values");

    group.bench_function("prefix_match", |b| {
        b.to_async(&rt).iter(|| async {
            sbql_core::query::suggest_distinct_values(
                &pool,
                "SELECT * FROM users",
                "name",
                "user_1",
                20,
            )
            .await
            .unwrap()
        })
    });

    group.bench_function("empty_prefix", |b| {
        b.to_async(&rt).iter(|| async {
            sbql_core::query::suggest_distinct_values(
                &pool,
                "SELECT * FROM users",
                "city",
                "",
                20,
            )
            .await
            .unwrap()
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// scaling: different row counts
// ---------------------------------------------------------------------------

fn bench_scaling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let pool_100 = rt.block_on(setup_db(100, 0));
    let pool_500 = rt.block_on(setup_db(500, 0));
    let pool_1000 = rt.block_on(setup_db(1000, 0));

    let mut group = c.benchmark_group("scaling");

    group.bench_function("100_rows", |b| {
        b.to_async(&rt).iter(|| async {
            execute_page(&pool_100, "SELECT * FROM users", 0)
                .await
                .unwrap()
        })
    });

    group.bench_function("500_rows", |b| {
        b.to_async(&rt).iter(|| async {
            execute_page(&pool_500, "SELECT * FROM users", 0)
                .await
                .unwrap()
        })
    });

    group.bench_function("1000_rows", |b| {
        b.to_async(&rt).iter(|| async {
            execute_page(&pool_1000, "SELECT * FROM users", 0)
                .await
                .unwrap()
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_execute_page,
    bench_full_lifecycle,
    bench_schema,
    bench_suggest_distinct_values,
    bench_scaling,
);
criterion_main!(benches);
