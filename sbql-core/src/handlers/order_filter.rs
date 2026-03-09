use crate::{query, query_builder, Core, CoreEvent, SortDirection};

pub(crate) async fn apply_order(
    core: &mut Core,
    column: String,
    direction: SortDirection,
) -> Vec<CoreEvent> {
    let base = match &core.effective_sql {
        Some(s) => s.clone(),
        None => return vec![CoreEvent::Error("No active query".into())],
    };
    let backend = match core.active_backend() {
        Ok(b) => b,
        Err(e) => return vec![CoreEvent::Error(e.to_string())],
    };
    let without_order = query_builder::clear_order(&base, backend).unwrap_or(base);
    match query_builder::apply_order(&without_order, &column, direction, backend) {
        Ok(new_sql) => {
            core.effective_sql = Some(new_sql);
            core.sort_state.clear();
            core.sort_state.insert(column, direction);
            core.execute_current_page(0).await
        }
        Err(e) => vec![CoreEvent::Error(e.to_string())],
    }
}

pub(crate) async fn clear_order(core: &mut Core) -> Vec<CoreEvent> {
    let effective = match &core.effective_sql {
        Some(s) => s.clone(),
        None => return vec![],
    };
    let backend = match core.active_backend() {
        Ok(b) => b,
        Err(e) => return vec![CoreEvent::Error(e.to_string())],
    };
    match query_builder::clear_order(&effective, backend) {
        Ok(new_sql) => {
            core.effective_sql = Some(new_sql);
            core.sort_state.clear();
            core.execute_current_page(0).await
        }
        Err(e) => vec![CoreEvent::Error(e.to_string())],
    }
}

pub(crate) async fn apply_filter(core: &mut Core, filter: String) -> Vec<CoreEvent> {
    let base = match &core.base_sql {
        Some(s) => s.clone(),
        None => return vec![CoreEvent::Error("No active query".into())],
    };
    let backend = match core.active_backend() {
        Ok(b) => b,
        Err(e) => return vec![CoreEvent::Error(e.to_string())],
    };
    let cols = if core.last_columns.is_empty() {
        None
    } else {
        Some(core.last_columns.as_slice())
    };
    match query_builder::apply_filter(&base, &filter, cols, backend) {
        Ok(filtered_sql) => {
            let final_sql = if let Some((col, &dir)) = core.sort_state.iter().next() {
                query_builder::apply_order(&filtered_sql, col, dir, backend).unwrap_or(filtered_sql)
            } else {
                filtered_sql
            };
            core.effective_sql = Some(final_sql);
            core.active_filter = Some(filter);
            core.execute_current_page(0).await
        }
        Err(e) => vec![CoreEvent::Error(e.to_string())],
    }
}

pub(crate) async fn clear_filter(core: &mut Core) -> Vec<CoreEvent> {
    core.active_filter = None;
    let base = match &core.base_sql {
        Some(s) => s.clone(),
        None => return vec![],
    };
    let backend = match core.active_backend() {
        Ok(b) => b,
        Err(e) => return vec![CoreEvent::Error(e.to_string())],
    };
    let final_sql = if let Some((col, &dir)) = core.sort_state.iter().next() {
        query_builder::apply_order(&base, col, dir, backend).unwrap_or(base)
    } else {
        base
    };
    core.effective_sql = Some(final_sql);
    core.execute_current_page(0).await
}

pub(crate) async fn suggest_filter_values(
    core: &mut Core,
    column: String,
    prefix: String,
    limit: usize,
    token: u64,
) -> Vec<CoreEvent> {
    let pool = match core.active_pool().await {
        Ok(p) => p,
        Err(e) => return vec![CoreEvent::Error(e.to_string())],
    };
    let base = match &core.base_sql {
        Some(s) => s.clone(),
        None => {
            return vec![CoreEvent::FilterSuggestions {
                items: Vec::new(),
                token,
            }]
        }
    };

    let column = core
        .last_columns
        .iter()
        .find(|c| c.eq_ignore_ascii_case(&column))
        .cloned()
        .unwrap_or(column);

    match query::suggest_distinct_values(&pool, &base, &column, &prefix, limit).await {
        Ok(items) => vec![CoreEvent::FilterSuggestions { items, token }],
        Err(e) => vec![CoreEvent::Error(e.to_string())],
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use crate::{ConnectionConfig, Core, CoreCommand, CoreEvent, SortDirection};

    /// Connect a Core to SQLite in-memory, create a table, insert data, and execute a query.
    async fn setup_sqlite_with_query() -> Core {
        let mut core = Core::default();
        let config = ConnectionConfig::new_sqlite("test", ":memory:");
        let id = config.id;
        core.connections.push(config);
        core.password_cache.insert(id, String::new());
        core.handle(CoreCommand::Connect(id)).await;

        // Create test data
        let pool = core.active_pool().await.unwrap();
        if let crate::pool::DbPool::Sqlite(sq) = &pool {
            sqlx::query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)")
                .execute(sq)
                .await
                .unwrap();
            sqlx::query("INSERT INTO users VALUES (1, 'Alice', 'alice@example.com')")
                .execute(sq)
                .await
                .unwrap();
            sqlx::query("INSERT INTO users VALUES (2, 'Bob', 'bob@example.com')")
                .execute(sq)
                .await
                .unwrap();
            sqlx::query("INSERT INTO users VALUES (3, 'Charlie', 'charlie@example.com')")
                .execute(sq)
                .await
                .unwrap();
        }

        // Execute initial query so base_sql is set
        core.handle(CoreCommand::ExecuteQuery {
            sql: "SELECT * FROM users".into(),
        })
        .await;

        core
    }

    #[tokio::test]
    async fn test_apply_order_no_effective_sql() {
        let mut core = Core::default();
        core.effective_sql = None;
        let events = core
            .handle(CoreCommand::ApplyOrder {
                column: "id".into(),
                direction: SortDirection::Ascending,
            })
            .await;
        assert!(matches!(&events[0], CoreEvent::Error(msg) if msg.contains("No active query")));
    }

    #[tokio::test]
    async fn test_apply_order_updates_sort_state() {
        let mut core = setup_sqlite_with_query().await;

        let events = core
            .handle(CoreCommand::ApplyOrder {
                column: "name".into(),
                direction: SortDirection::Descending,
            })
            .await;

        // Should get a QueryResult back
        assert!(events
            .iter()
            .any(|e| matches!(e, CoreEvent::QueryResult(_))));
        // sort_state should be updated
        assert_eq!(
            core.sort_state.get("name"),
            Some(&SortDirection::Descending)
        );
    }

    #[tokio::test]
    async fn test_clear_order_restores_base() {
        let mut core = setup_sqlite_with_query().await;

        // Apply order first
        core.handle(CoreCommand::ApplyOrder {
            column: "name".into(),
            direction: SortDirection::Ascending,
        })
        .await;
        assert!(!core.sort_state.is_empty());

        // Clear order
        let events = core.handle(CoreCommand::ClearOrder).await;
        assert!(events
            .iter()
            .any(|e| matches!(e, CoreEvent::QueryResult(_))));
        assert!(core.sort_state.is_empty());
    }

    #[tokio::test]
    async fn test_apply_filter_sets_active_filter() {
        let mut core = setup_sqlite_with_query().await;

        let events = core
            .handle(CoreCommand::ApplyFilter {
                query: "name:Alice".into(),
            })
            .await;
        assert!(events
            .iter()
            .any(|e| matches!(e, CoreEvent::QueryResult(_))));
        assert_eq!(core.active_filter, Some("name:Alice".to_string()));
    }

    #[tokio::test]
    async fn test_clear_filter_preserves_sort() {
        let mut core = setup_sqlite_with_query().await;

        // Apply sort + filter
        core.handle(CoreCommand::ApplyOrder {
            column: "name".into(),
            direction: SortDirection::Ascending,
        })
        .await;
        core.handle(CoreCommand::ApplyFilter {
            query: "alice".into(),
        })
        .await;
        assert!(core.active_filter.is_some());

        // Clear filter - sort should be preserved
        core.handle(CoreCommand::ClearFilter).await;
        assert!(core.active_filter.is_none());
        assert_eq!(core.sort_state.get("name"), Some(&SortDirection::Ascending));
    }

    #[tokio::test]
    async fn test_suggest_filter_values_no_base_sql() {
        let mut core = Core::default();
        let config = ConnectionConfig::new_sqlite("test", ":memory:");
        let id = config.id;
        core.connections.push(config);
        core.password_cache.insert(id, String::new());
        core.handle(CoreCommand::Connect(id)).await;
        // base_sql is None → empty suggestions
        let events = core
            .handle(CoreCommand::SuggestFilterValues {
                column: "name".into(),
                prefix: "".into(),
                limit: 10,
                token: 42,
            })
            .await;
        match &events[0] {
            CoreEvent::FilterSuggestions { items, token } => {
                assert!(items.is_empty());
                assert_eq!(*token, 42);
            }
            _ => panic!("Expected FilterSuggestions"),
        }
    }
}
