use sbql_core::{ConnectionConfig, Core, CoreCommand, CoreEvent};

#[tokio::main]
async fn main() {
    let mut core = Core::default();

    let config = ConnectionConfig::new_mysql("test-mysql", "localhost", 3306, "root", "testdb");
    let id = config.id;
    core.handle(CoreCommand::SaveConnection {
        config,
        password: Some("root123".into()),
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
                    println!("Row {i}: {:?}", row);
                }
            }
            CoreEvent::Error(e) => println!("QUERY ERROR: {e}"),
            _ => {}
        }
    }

    let _ = core.handle(CoreCommand::Disconnect(id)).await;
}
