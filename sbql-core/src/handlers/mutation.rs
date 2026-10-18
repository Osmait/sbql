use crate::{schema, Core, CoreEvent, RowChange};

pub(crate) async fn update_cell(
    core: &mut Core,
    schema_name: String,
    table: String,
    pk: Vec<(String, String)>,
    target_col: String,
    new_val: String,
) -> Vec<CoreEvent> {
    let pool = match core.active_pool().await {
        Ok(p) => p,
        Err(e) => return vec![CoreEvent::error(&e)],
    };
    match schema::execute_cell_update(&pool, &schema_name, &table, &pk, &target_col, &new_val).await
    {
        Ok(()) => vec![CoreEvent::CellUpdated],
        Err(e) => vec![CoreEvent::error(&e)],
    }
}

pub(crate) async fn delete_row(
    core: &mut Core,
    schema_name: String,
    table: String,
    pk: Vec<(String, String)>,
) -> Vec<CoreEvent> {
    let pool = match core.active_pool().await {
        Ok(p) => p,
        Err(e) => return vec![CoreEvent::error(&e)],
    };
    match schema::execute_row_delete(&pool, &schema_name, &table, &pk).await {
        Ok(()) => vec![CoreEvent::RowDeleted],
        Err(e) => vec![CoreEvent::error(&e)],
    }
}

pub(crate) async fn commit_changes(core: &mut Core, changes: Vec<RowChange>) -> Vec<CoreEvent> {
    let pool = match core.active_pool().await {
        Ok(p) => p,
        Err(e) => return vec![CoreEvent::error(&e)],
    };
    match schema::execute_changes(&pool, &changes).await {
        Ok(summary) => vec![CoreEvent::ChangesCommitted(summary)],
        Err(e) => vec![CoreEvent::error(&e)],
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
                pk: vec![("id".into(), "1".into())],
                target_col: "name".into(),
                new_val: "Bob".into(),
            })
            .await;
        assert!(
            matches!(&events[0], CoreEvent::Error(e) if e.message.contains("No active connection"))
        );
    }

    #[tokio::test]
    async fn test_delete_row_no_connection() {
        let mut core = Core::default();
        let events = core
            .handle(CoreCommand::DeleteRow {
                schema: "main".into(),
                table: "users".into(),
                pk: vec![("id".into(), "1".into())],
            })
            .await;
        assert!(
            matches!(&events[0], CoreEvent::Error(e) if e.message.contains("No active connection"))
        );
    }

    #[tokio::test]
    async fn test_update_cell_round_trip() {
        let (mut core, _id) = setup_sqlite_core().await;

        let events = core
            .handle(CoreCommand::UpdateCell {
                schema: "main".into(),
                table: "test_table".into(),
                pk: vec![("id".into(), "1".into())],
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
                pk: vec![("id".into(), "1".into())],
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

    fn update(id: &str, name: &str) -> crate::RowChange {
        crate::RowChange::Update {
            schema: "main".into(),
            table: "test_table".into(),
            pk: vec![("id".into(), id.into())],
            target_col: "name".into(),
            new_val: name.into(),
        }
    }

    fn delete(id: &str) -> crate::RowChange {
        crate::RowChange::Delete {
            schema: "main".into(),
            table: "test_table".into(),
            pk: vec![("id".into(), id.into())],
        }
    }

    /// Run setup SQL straight on the pool — `ExecuteQuery` pages its input,
    /// which is not what DDL wants.
    async fn exec(core: &mut Core, sql: &str) {
        let pool = core.active_pool().await.unwrap();
        if let crate::pool::DbPool::Sqlite(sq) = &pool {
            sqlx::query(sql).execute(sq).await.unwrap();
        }
    }

    async fn names(core: &mut Core) -> Vec<Vec<String>> {
        let events = core
            .handle(CoreCommand::ExecuteQuery {
                sql: "SELECT id, name FROM test_table ORDER BY id".into(),
            })
            .await;
        match &events[0] {
            CoreEvent::QueryResult(r) => r.rows.clone(),
            other => panic!("Expected QueryResult, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn commit_changes_applies_edits_and_deletes() {
        let (mut core, _id) = setup_sqlite_core().await;
        exec(
            &mut core,
            "INSERT INTO test_table (id, name) VALUES (2, 'Bob')",
        )
        .await;

        let events = core
            .handle(CoreCommand::CommitChanges {
                changes: vec![update("1", "Alicia"), delete("2")],
            })
            .await;

        assert!(matches!(
            &events[0],
            CoreEvent::ChangesCommitted(crate::CommitSummary {
                updated: 1,
                deleted: 1
            })
        ));
        assert_eq!(
            names(&mut core).await,
            vec![vec!["1".to_string(), "Alicia".into()]]
        );
    }

    /// A delete blocked by a foreign key used to leave the edits before it
    /// applied while the TUI announced the whole commit as done.
    #[tokio::test]
    async fn commit_changes_is_all_or_nothing() {
        let (mut core, _id) = setup_sqlite_core().await;
        for sql in [
            "INSERT INTO test_table (id, name) VALUES (2, 'Bob')",
            "CREATE TABLE child (id INTEGER PRIMARY KEY, parent INTEGER NOT NULL REFERENCES test_table(id))",
            "INSERT INTO child (id, parent) VALUES (1, 1)",
        ] {
            exec(&mut core, sql).await;
        }

        let events = core
            .handle(CoreCommand::CommitChanges {
                changes: vec![update("2", "Roberto"), delete("1")],
            })
            .await;

        assert!(
            matches!(&events[0], CoreEvent::Error(e) if e.message.contains("FOREIGN KEY")),
            "got {events:?}"
        );
        assert_eq!(
            names(&mut core).await,
            vec![
                vec!["1".to_string(), "Alice".into()],
                vec!["2".to_string(), "Bob".into()],
            ]
        );
    }
}
