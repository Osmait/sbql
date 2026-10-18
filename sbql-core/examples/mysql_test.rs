// Benchmark harness: `unwrap` on setup is how a bench reports it cannot run.
// `clippy.toml` exempts `#[cfg(test)]` modules from the workspace panic lints,
// but not benches or examples.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sbql_core::{ConnectionConfig, Core, CoreCommand, CoreEvent};

// Printing *is* what this example does — it is a hand-run probe against a local
// MySQL, read off a normal terminal. The workspace `print_stdout` lint is aimed
// at the TUI, where a stray write lands in the middle of the alternate screen;
// nothing here holds a screen, so the exemption is scoped to this one function
// rather than turned off for the crate.
#[tokio::main]
#[allow(clippy::print_stdout)]
async fn main() {
    let mut core = Core::default();

    let config = ConnectionConfig::new_mysql("test-mysql", "localhost", 3306, "root", "testdb");
    let id = config.id;
    core.handle(CoreCommand::SaveConnection {
        config,
        password: Some("root123".into()),
        ssh_password: None,
    })
    .await;

    let events = core.handle(CoreCommand::Connect(id)).await;
    for e in &events {
        if let CoreEvent::Error(msg) = e {
            println!("CONNECT ERROR: {msg}");
            return;
        }
    }
    println!("Connected OK");

    let events = core.handle(CoreCommand::ListTables).await;
    for event in &events {
        match event {
            CoreEvent::TableList(tables) => {
                println!("Got {} tables:", tables.len());
                for t in tables {
                    println!("  schema={:?}  name={:?}", t.schema, t.name);
                }
            }
            CoreEvent::Error(e) => println!("ERROR: {e}"),
            _ => {}
        }
    }

    let events = core
        .handle(CoreCommand::ExecuteQuery {
            sql: "SELECT * FROM testdb.users".into(),
        })
        .await;
    for event in &events {
        match event {
            CoreEvent::QueryResult(r) => {
                println!("Query: {} cols, {} rows", r.columns.len(), r.rows.len());
                println!("Columns: {:?}", r.columns);
                for (i, row) in r.rows.iter().enumerate().take(2) {
                    println!("Row {i}: {row:?}");
                }
            }
            CoreEvent::Error(e) => println!("QUERY ERROR: {e}"),
            _ => {}
        }
    }

    // Teardown; the events it reports have nowhere left to go.
    drop(core.handle(CoreCommand::Disconnect(id)).await);
}
