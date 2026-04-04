//! Pure CPU benchmarks for query manipulation functions (no database required).

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use sbql_core::pool::DbBackend;
use sbql_core::query::{build_paginated_sql, redis_value_to_query_result, tokenize_redis_command};
use sbql_core::query_builder::{apply_filter, apply_order, clear_order, SortDirection};

// ---------------------------------------------------------------------------
// apply_order
// ---------------------------------------------------------------------------

fn bench_apply_order(c: &mut Criterion) {
    let mut group = c.benchmark_group("apply_order");

    group.bench_function("postgres_asc", |b| {
        b.iter(|| {
            apply_order(
                black_box("SELECT * FROM users"),
                black_box("name"),
                black_box(SortDirection::Ascending),
                black_box(DbBackend::Postgres),
            )
        })
    });

    group.bench_function("sqlite_desc", |b| {
        b.iter(|| {
            apply_order(
                black_box("SELECT id, email FROM users WHERE active = true"),
                black_box("created_at"),
                black_box(SortDirection::Descending),
                black_box(DbBackend::Sqlite),
            )
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// clear_order
// ---------------------------------------------------------------------------

fn bench_clear_order(c: &mut Criterion) {
    let mut group = c.benchmark_group("clear_order");

    group.bench_function("with_order_by", |b| {
        b.iter(|| {
            clear_order(
                black_box("SELECT * FROM users ORDER BY name ASC"),
                black_box(DbBackend::Postgres),
            )
        })
    });

    group.bench_function("without_order_by", |b| {
        b.iter(|| {
            clear_order(
                black_box("SELECT * FROM users WHERE active = true"),
                black_box(DbBackend::Postgres),
            )
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// apply_filter
// ---------------------------------------------------------------------------

fn bench_apply_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("apply_filter");

    let cols: Vec<String> = vec![
        "name".into(),
        "email".into(),
        "status".into(),
        "city".into(),
    ];

    group.bench_function("column_specific_pg", |b| {
        b.iter(|| {
            apply_filter(
                black_box("SELECT * FROM users"),
                black_box("status:active"),
                black_box(None),
                black_box(DbBackend::Postgres),
            )
        })
    });

    group.bench_function("global_pg", |b| {
        b.iter(|| {
            apply_filter(
                black_box("SELECT * FROM users"),
                black_box("alice"),
                black_box(Some(cols.as_slice())),
                black_box(DbBackend::Postgres),
            )
        })
    });

    group.bench_function("column_specific_sqlite", |b| {
        b.iter(|| {
            apply_filter(
                black_box("SELECT * FROM users"),
                black_box("name:Bob"),
                black_box(None),
                black_box(DbBackend::Sqlite),
            )
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// build_paginated_sql
// ---------------------------------------------------------------------------

fn bench_build_paginated_sql(c: &mut Criterion) {
    let mut group = c.benchmark_group("build_paginated_sql");

    group.bench_function("page_0", |b| {
        b.iter(|| build_paginated_sql(black_box("SELECT * FROM users"), black_box(0)))
    });

    group.bench_function("page_10", |b| {
        b.iter(|| build_paginated_sql(black_box("SELECT * FROM users"), black_box(10)))
    });

    group.bench_function("already_limited", |b| {
        b.iter(|| {
            build_paginated_sql(black_box("SELECT * FROM users LIMIT 50"), black_box(0))
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// tokenize_redis_command
// ---------------------------------------------------------------------------

fn bench_tokenize_redis_command(c: &mut Criterion) {
    let mut group = c.benchmark_group("tokenize_redis_command");

    group.bench_function("simple", |b| {
        b.iter(|| tokenize_redis_command(black_box("GET mykey")))
    });

    group.bench_function("quoted", |b| {
        b.iter(|| tokenize_redis_command(black_box(r#"SET mykey "hello world""#)))
    });

    group.bench_function("complex", |b| {
        b.iter(|| {
            tokenize_redis_command(black_box(
                r#"HSET user:1000 name "John Doe" email "john@example.com" age 30"#,
            ))
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// redis_value_to_query_result
// ---------------------------------------------------------------------------

fn bench_redis_value_to_query_result(c: &mut Criterion) {
    let mut group = c.benchmark_group("redis_value_to_query_result");

    group.bench_function("simple_string", |b| {
        let val = redis::Value::SimpleString("OK".into());
        b.iter(|| redis_value_to_query_result(black_box(&val)))
    });

    group.bench_function("int", |b| {
        let val = redis::Value::Int(42);
        b.iter(|| redis_value_to_query_result(black_box(&val)))
    });

    group.bench_function("hash_array", |b| {
        let val = redis::Value::Array(vec![
            redis::Value::BulkString(b"field1".to_vec()),
            redis::Value::BulkString(b"value1".to_vec()),
            redis::Value::BulkString(b"field2".to_vec()),
            redis::Value::BulkString(b"value2".to_vec()),
        ]);
        b.iter(|| redis_value_to_query_result(black_box(&val)))
    });

    group.bench_function("large_array", |b| {
        let val = redis::Value::Array(
            (0..100)
                .map(|i| redis::Value::BulkString(format!("item-{i}").into_bytes()))
                .collect(),
        );
        b.iter(|| redis_value_to_query_result(black_box(&val)))
    });

    group.bench_function("nil", |b| {
        let val = redis::Value::Nil;
        b.iter(|| redis_value_to_query_result(black_box(&val)))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_apply_order,
    bench_clear_order,
    bench_apply_filter,
    bench_build_paginated_sql,
    bench_tokenize_redis_command,
    bench_redis_value_to_query_result,
);
criterion_main!(benches);
