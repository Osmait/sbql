//! Redis integration benchmarks using testcontainers.
//!
//! Requires Docker running. These benchmarks measure real Redis round-trips
//! including command tokenization and value-to-QueryResult conversion.
//!
//! Run with: cargo bench --package sbql-core --bench redis_integration

use criterion::{criterion_group, criterion_main, Criterion};
use tokio::runtime::Runtime;

use sbql_core::pool::DbPool;
use sbql_core::query::execute_page;

use testcontainers::runners::AsyncRunner;
use testcontainers_modules::redis::Redis;

/// Spin up a Redis container and seed it. Returns pool + container handle.
async fn setup_redis() -> (DbPool, testcontainers::ContainerAsync<Redis>) {
    let container = Redis::default().start().await.unwrap();
    let host_ip = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(6379).await.unwrap();

    let url = format!("redis://{}:{}", host_ip, host_port);
    let client = redis::Client::open(url.as_str()).unwrap();
    let cm = redis::aio::ConnectionManager::new(client).await.unwrap();
    let pool = DbPool::Redis(Box::new(cm));

    // Seed data
    for i in 0..100 {
        execute_page(&pool, &format!("SET key:{i} value_{i}"), 0)
            .await
            .unwrap();
    }
    for i in 0..20 {
        execute_page(
            &pool,
            &format!("HSET user:1000 field_{i} value_{i}"),
            0,
        )
        .await
        .unwrap();
    }
    for i in 0..50 {
        execute_page(&pool, &format!("LPUSH mylist item_{i}"), 0)
            .await
            .unwrap();
    }

    (pool, container)
}

fn bench_redis(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (pool, _container) = rt.block_on(setup_redis());

    // --- Simple commands ---
    let mut group = c.benchmark_group("redis_simple");
    group.sample_size(50);

    group.bench_function("get_existing", |b| {
        b.to_async(&rt).iter(|| async {
            execute_page(&pool, "GET key:42", 0).await.unwrap()
        })
    });

    group.bench_function("get_missing", |b| {
        b.to_async(&rt).iter(|| async {
            execute_page(&pool, "GET nonexistent", 0).await.unwrap()
        })
    });

    group.bench_function("set", |b| {
        b.to_async(&rt).iter(|| async {
            execute_page(&pool, "SET bench:key bench_value", 0)
                .await
                .unwrap()
        })
    });

    group.bench_function("ping", |b| {
        b.to_async(&rt)
            .iter(|| async { execute_page(&pool, "PING", 0).await.unwrap() })
    });

    group.finish();

    // --- Hash commands ---
    let mut group = c.benchmark_group("redis_hash");
    group.sample_size(50);

    group.bench_function("hgetall_20_fields", |b| {
        b.to_async(&rt).iter(|| async {
            execute_page(&pool, "HGETALL user:1000", 0)
                .await
                .unwrap()
        })
    });

    group.bench_function("hget_single", |b| {
        b.to_async(&rt).iter(|| async {
            execute_page(&pool, "HGET user:1000 field_5", 0)
                .await
                .unwrap()
        })
    });

    group.bench_function("hset", |b| {
        b.to_async(&rt).iter(|| async {
            execute_page(
                &pool,
                r#"HSET bench:hash name "John Doe" age 30"#,
                0,
            )
            .await
            .unwrap()
        })
    });

    group.finish();

    // --- List commands ---
    let mut group = c.benchmark_group("redis_list");
    group.sample_size(50);

    group.bench_function("lrange_50_items", |b| {
        b.to_async(&rt).iter(|| async {
            execute_page(&pool, "LRANGE mylist 0 -1", 0)
                .await
                .unwrap()
        })
    });

    group.bench_function("lrange_10_items", |b| {
        b.to_async(&rt).iter(|| async {
            execute_page(&pool, "LRANGE mylist 0 9", 0)
                .await
                .unwrap()
        })
    });

    group.bench_function("llen", |b| {
        b.to_async(&rt).iter(|| async {
            execute_page(&pool, "LLEN mylist", 0).await.unwrap()
        })
    });

    group.finish();

    // --- Bulk operations ---
    let mut group = c.benchmark_group("redis_bulk");
    group.sample_size(50);

    group.bench_function("keys_star", |b| {
        b.to_async(&rt)
            .iter(|| async { execute_page(&pool, "KEYS *", 0).await.unwrap() })
    });

    group.bench_function("dbsize", |b| {
        b.to_async(&rt)
            .iter(|| async { execute_page(&pool, "DBSIZE", 0).await.unwrap() })
    });

    group.finish();
}

criterion_group!(benches, bench_redis);
criterion_main!(benches);
