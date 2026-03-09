use crate::{schema, Core, CoreEvent};

pub(crate) async fn update_cell(
    core: &mut Core,
    schema_name: String,
    table: String,
    pk_col: String,
    pk_val: String,
    target_col: String,
    new_val: String,
) -> Vec<CoreEvent> {
    let pool = match core.active_pool().await {
        Ok(p) => p,
        Err(e) => return vec![CoreEvent::Error(e.to_string())],
    };
    match schema::execute_cell_update(
        &pool,
        &schema_name,
        &table,
        &pk_col,
        &pk_val,
        &target_col,
        &new_val,
    )
    .await
    {
        Ok(()) => vec![CoreEvent::CellUpdated],
        Err(e) => vec![CoreEvent::Error(e.to_string())],
    }
}

pub(crate) async fn delete_row(
    core: &mut Core,
    schema_name: String,
    table: String,
    pk_col: String,
    pk_val: String,
) -> Vec<CoreEvent> {
    let pool = match core.active_pool().await {
        Ok(p) => p,
        Err(e) => return vec![CoreEvent::Error(e.to_string())],
    };
    match schema::execute_row_delete(&pool, &schema_name, &table, &pk_col, &pk_val).await {
        Ok(()) => vec![CoreEvent::RowDeleted],
        Err(e) => vec![CoreEvent::Error(e.to_string())],
    }
}

#[cfg(test)]
mod tests {
    use crate::{ConnectionConfig, Core, CoreCommand, CoreEvent};

    /// Connect a Core to an in-memory SQLite database and create a test table.
    async fn setup_sqlite_core() -> (Core, uuid::Uuid) {
        let mut core = Core::default();
        let config = ConnectionConfig::new_sqlite("test", ":memory:");
        let id = config.id;
        core.connections.push(config);
        core.password_cache.insert(id, String::new());

        let events = core.handle(CoreCommand::Connect(id)).await;
        assert!(matches!(&events[0], CoreEvent::Connected(_)));

        // Create a test table and insert a row
        let pool = core.active_pool().await.unwrap();
        if let crate::pool::DbPool::Sqlite(sq) = &pool {
            sqlx::query("CREATE TABLE test_table (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
                .execute(sq)
                .await
                .unwrap();
            sqlx::query("INSERT INTO test_table (id, name) VALUES (1, 'Alice')")
                .execute(sq)
                .await
                .unwrap();
        }

        (core, id)
    }

    #[tokio::test]
    async fn test_update_cell_no_connection() {
        let mut core = Core::default();
        let events = core
            .handle(CoreCommand::UpdateCell {
                schema: "main".into(),
                table: "users".into(),
                pk_col: "id".into(),
                pk_val: "1".into(),
                target_col: "name".into(),
                new_val: "Bob".into(),
            })
            .await;
        assert!(
            matches!(&events[0], CoreEvent::Error(msg) if msg.contains("No active connection"))
        );
    }

    #[tokio::test]
    async fn test_delete_row_no_connection() {
        let mut core = Core::default();
        let events = core
            .handle(CoreCommand::DeleteRow {
                schema: "main".into(),
                table: "users".into(),
                pk_col: "id".into(),
                pk_val: "1".into(),
            })
            .await;
        assert!(
            matches!(&events[0], CoreEvent::Error(msg) if msg.contains("No active connection"))
        );
    }

    #[tokio::test]
    async fn test_update_cell_round_trip() {
        let (mut core, _id) = setup_sqlite_core().await;

        let events = core
            .handle(CoreCommand::UpdateCell {
                schema: "main".into(),
                table: "test_table".into(),
                pk_col: "id".into(),
                pk_val: "1".into(),
                target_col: "name".into(),
                new_val: "Bob".into(),
            })
            .await;
        assert!(matches!(&events[0], CoreEvent::CellUpdated));

        // Verify the update
        let events = core
            .handle(CoreCommand::ExecuteQuery {
                sql: "SELECT name FROM test_table WHERE id = 1".into(),
            })
            .await;
        if let CoreEvent::QueryResult(r) = &events[0] {
            assert_eq!(r.rows[0][0], "Bob");
        } else {
            panic!("Expected QueryResult");
        }
    }

    #[tokio::test]
    async fn test_delete_row_round_trip() {
        let (mut core, _id) = setup_sqlite_core().await;

        let events = core
            .handle(CoreCommand::DeleteRow {
                schema: "main".into(),
                table: "test_table".into(),
                pk_col: "id".into(),
                pk_val: "1".into(),
            })
            .await;
        assert!(matches!(&events[0], CoreEvent::RowDeleted));

        // Verify the row is gone
        let events = core
            .handle(CoreCommand::ExecuteQuery {
                sql: "SELECT * FROM test_table".into(),
            })
            .await;
        if let CoreEvent::QueryResult(r) = &events[0] {
            assert!(r.rows.is_empty());
        } else {
            panic!("Expected QueryResult");
        }
    }
}
