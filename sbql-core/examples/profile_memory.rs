//! Heap profiling workload using dhat.
//!
//! Run with:
//!   cargo run --package sbql-core --example profile_memory --features dhat-heap
//!
//! Produces `dhat-heap.json` in the current directory.
//! View at: https://nnethercote.github.io/dh_view/dh_view.html

// Benchmark harness: `unwrap` on setup is how a bench reports it cannot run.
// `clippy.toml` exempts `#[cfg(test)]` modules from the workspace panic lints,
// but not benches or examples.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use sbql_core::pool::{DbBackend, DbPool};
use sbql_core::query::{build_paginated_sql, execute_page, tokenize_redis_command};
use sbql_core::query_builder::{apply_filter, apply_order, clear_order, SortDirection};
use sbql_core::schema::{list_tables, load_diagram};
use sqlx::SqlitePool;

async fn setup_db() -> DbPool {
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

    for i in 0..500 {
        sqlx::query("INSERT INTO users (name, email, active, city) VALUES (?, ?, ?, ?)")
            .bind(format!("user_{i}"))
            .bind(format!("user_{i}@example.com"))
            .bind(if i % 5 == 0 { 0 } else { 1 })
            .bind(["New York", "London", "Tokyo", "Berlin", "Paris"][i % 5])
            .execute(&pool)
            .await
            .unwrap();
    }

    for i in 0..1000 {
        let user_id = (i % 500) + 1;
        sqlx::query("INSERT INTO posts (user_id, title, body) VALUES (?, ?, ?)")
            .bind(user_id as i64)
            .bind(format!("Post title {i}"))
            .bind(format!("Body content for post {i} with some extra text."))
            .execute(&pool)
            .await
            .unwrap();
    }

    DbPool::Sqlite(pool)
}

/// Every result below is thrown away with `drop(..)` on purpose: this is a
/// workload, not a test, and what is being measured is the allocation each call
/// makes on the way to its answer.
///
/// `drop(x)` rather than the `let _unused = x` the lint also offers, because
/// the two are not equivalent here. `_unused` lives to the end of `main`, so
/// every result would still be alive when the next one is allocated and the
/// heap peak dhat reports would be the sum of the whole workload instead of its
/// true high-water mark. `drop` frees at the same point the old `let _ =` did.
#[tokio::main]
#[allow(clippy::print_stdout)] // An example run from a shell; nothing holds the screen.
async fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    let pool = setup_db().await;

    // --- Pagination workload ---
    for page in 0..5 {
        drop(execute_page(&pool, "SELECT * FROM users", page).await);
    }
    for page in 0..10 {
        drop(execute_page(&pool, "SELECT * FROM posts", page).await);
    }

    // --- SQL manipulation workload ---
    let base_sql = "SELECT * FROM users";
    for col in &["name", "email", "city", "id"] {
        let ordered = apply_order(base_sql, col, SortDirection::Ascending, DbBackend::Sqlite)
            .unwrap_or_default();
        drop(clear_order(&ordered, DbBackend::Sqlite));
    }

    let cols: Vec<String> = vec!["name".into(), "email".into(), "city".into()];
    for q in &["alice", "user_1", "Tokyo", "status:active", "name:Bob"] {
        drop(apply_filter(base_sql, q, Some(&cols), DbBackend::Sqlite));
    }

    for page in 0..5 {
        drop(build_paginated_sql("SELECT * FROM users", page));
    }

    // --- Schema introspection ---
    drop(list_tables(&pool).await);
    drop(load_diagram(&pool).await);

    // --- Join queries ---
    drop(
        execute_page(
            &pool,
            "SELECT p.id, p.title, u.name FROM posts p JOIN users u ON p.user_id = u.id",
            0,
        )
        .await,
    );

    // --- Redis tokenizer (pure CPU) ---
    for _ in 0..100 {
        drop(tokenize_redis_command(
            "HSET user:1000 name \"John Doe\" email \"john@example.com\"",
        ));
    }

    println!("Profiling workload complete.");
    #[cfg(feature = "dhat-heap")]
    println!("dhat-heap.json written. Open at https://nnethercote.github.io/dh_view/dh_view.html");
}
