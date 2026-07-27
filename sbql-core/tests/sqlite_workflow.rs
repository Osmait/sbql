//! End-to-end walk through the `Core` command/event loop using only SQLite.
//!
//! This is the one integration test that needs nothing from the host: no
//! Docker, and no OS credential store. SQLite connections never touch the
//! keyring — `save_password` returns early and `load_password` yields an empty
//! string — so this is the quickest way to confirm the whole stack works on a
//! platform, including a Linux box with no Secret Service running.
//!
//! ```bash
//! cargo test -p sbql-core --test sqlite_workflow
//! ```

// Integration test: an `unwrap` that fails here *is* the test failing, which is
// what it is for. `clippy.toml` exempts `#[cfg(test)]` modules from the panic
// lints in the workspace `Cargo.toml`, but not files under `tests/`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sbql_core::{config::CONFIG_DIR_ENV, Core, CoreCommand, CoreEvent};

/// Build a throwaway SQLite database plus a config file pointing at it, and
/// return the temp dir holding both (kept alive for the test's duration).
async fn scratch_workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");
    let db_path = dir.path().join("demo.db");

    seed_database(&db_path).await;

    let config = format!(
        r#"[[connections]]
id = "11111111-2222-3333-4444-555555555555"
name = "demo-sqlite"
backend = "sqlite"
host = ""
port = 0
user = ""
database = ""
ssl_mode = "prefer"
file_path = "{}"
ssh_enabled = false
ssh_host = ""
ssh_port = 22
ssh_user = ""
ssh_auth_method = ""
"#,
        db_path.display()
    );
    std::fs::write(dir.path().join("connections.toml"), config).expect("write config");

    std::env::set_var(CONFIG_DIR_ENV, dir.path());
    dir
}

/// Seed the database through sqlx, so the test needs no extra dependency.
async fn seed_database(db_path: &std::path::Path) {
    use sqlx::sqlite::SqlitePoolOptions;

    let url = format!("sqlite://{}?mode=rwc", db_path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("open sqlite");

    for stmt in [
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT NOT NULL)",
        "CREATE TABLE posts (id INTEGER PRIMARY KEY, \
         user_id INTEGER NOT NULL REFERENCES users(id), title TEXT NOT NULL)",
        "INSERT INTO users (name, email) VALUES \
         ('Alice','alice@example.com'), ('Bob','bob@example.com'), ('Carmen','c@example.com')",
        "INSERT INTO posts (user_id, title) VALUES (1,'first'), (1,'second'), (2,'third')",
    ] {
        sqlx::query(stmt).execute(&pool).await.expect("seed");
    }
}

#[tokio::test]
async fn full_workflow_runs_without_a_credential_store() {
    let _scratch = scratch_workspace().await;

    // Load saved connections from disk, exactly as the TUI does at startup.
    let mut core = Core::new();
    let cfg = core
        .connections
        .first()
        .cloned()
        .expect("demo connection should load from the config file");
    assert_eq!(cfg.name, "demo-sqlite");

    let events = core.handle(CoreCommand::Connect(cfg.id)).await;
    assert!(
        matches!(events.first(), Some(CoreEvent::Connected(_))),
        "connect failed: {events:?}"
    );

    let events = core.handle(CoreCommand::ListTables).await;
    let tables = events
        .iter()
        .find_map(|e| match e {
            CoreEvent::TableList(t) => Some(t),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no table list: {events:?}"));
    assert_eq!(tables.len(), 2, "expected users and posts: {tables:?}");

    let events = core
        .handle(CoreCommand::ExecuteQuery {
            sql: "SELECT u.name, count(p.id) AS posts FROM users u \
                  LEFT JOIN posts p ON p.user_id = u.id GROUP BY u.id ORDER BY u.name"
                .into(),
        })
        .await;
    let result = events
        .iter()
        .find_map(|e| match e {
            CoreEvent::QueryResult(r) => Some(r),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no query result: {events:?}"));
    assert_eq!(result.columns, vec!["name", "posts"]);
    assert_eq!(result.rows.len(), 3);
    assert_eq!(result.rows[0][0], "Alice");
    assert_eq!(result.rows[0][1], "2");

    // The diagram exercises schema + foreign-key introspection.
    let events = core.handle(CoreCommand::LoadDiagram).await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, CoreEvent::DiagramLoaded(_))),
        "diagram failed: {events:?}"
    );
}
