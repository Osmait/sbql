use crate::{query, Core, CoreEvent};

pub(crate) async fn execute(core: &mut Core, sql: String) -> Vec<CoreEvent> {
    core.base_sql = Some(sql.clone());
    core.effective_sql = Some(sql);
    core.active_filter = None;

    // A dropped sort goes out before the rows, so a client cannot draw the new
    // result set under the previous query's sort indicator.
    let mut events: Vec<CoreEvent> = core.set_sort(None).into_iter().collect();
    events.extend(core.execute_current_page(0).await);
    events
}

pub(crate) async fn fetch_page(core: &mut Core, page: usize) -> Vec<CoreEvent> {
    core.execute_current_page(page).await
}

/// Count the rows the current query would return.
///
/// Always answers with [`CoreEvent::TotalCount`], never an error: the count is
/// advisory, and "no query yet" or "not connected" are as un-actionable to the
/// client as a timeout — all of them mean the same thing, that there is no
/// number to show. Raising them as errors would put a red toast in front of
/// the user for a lookup they never asked for.
pub(crate) async fn fetch_total_count(core: &Core) -> Vec<CoreEvent> {
    let Some(sql) = core.effective_sql.clone() else {
        return vec![CoreEvent::TotalCount(None)];
    };
    let Ok(pool) = core.active_pool().await else {
        return vec![CoreEvent::TotalCount(None)];
    };
    vec![CoreEvent::TotalCount(query::total_count(&pool, &sql).await)]
}

#[cfg(test)]
mod tests {
    use crate::{ConnectionConfig, Core, CoreCommand, CoreEvent};

    /// A connected Core over in-memory SQLite holding three rows.
    async fn core_with_three_users() -> Core {
        let mut core = Core::default();
        let config = ConnectionConfig::new_sqlite("count_test", ":memory:");
        let id = config.id;
        core.connections.push(config);
        core.password_cache.insert(id, String::new());
        core.handle(CoreCommand::Connect(id)).await;

        let pool = core.active_pool().await.unwrap();
        if let crate::pool::DbPool::Sqlite(sq) = &pool {
            sqlx::query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)")
                .execute(sq)
                .await
                .unwrap();
            for (id, name) in [(1, "Alice"), (2, "Bob"), (3, "Charlie")] {
                sqlx::query("INSERT INTO users VALUES (?, ?)")
                    .bind(id)
                    .bind(name)
                    .execute(sq)
                    .await
                    .unwrap();
            }
        }
        core
    }

    fn total_count_of(events: &[CoreEvent]) -> Option<u64> {
        events
            .iter()
            .find_map(|e| match e {
                CoreEvent::TotalCount(c) => Some(*c),
                _ => None,
            })
            .expect("expected a TotalCount event")
    }

    /// Page 0 no longer waits on `COUNT(*)`. It used to, which is up to three
    /// seconds of nothing on screen for a number the TUI never renders.
    #[tokio::test]
    async fn executing_a_query_returns_the_page_without_counting() {
        let mut core = core_with_three_users().await;

        let events = core
            .handle(CoreCommand::ExecuteQuery {
                sql: "SELECT * FROM users".into(),
            })
            .await;

        let result = events
            .iter()
            .find_map(|e| match e {
                CoreEvent::QueryResult(r) => Some(r),
                _ => None,
            })
            .expect("expected a QueryResult");
        assert_eq!(result.rows.len(), 3);
        assert_eq!(
            result.total_count, None,
            "the page must not carry a count it had to wait for"
        );
    }

    /// The count is still available — just asked for separately.
    #[tokio::test]
    async fn the_count_is_available_on_request() {
        let mut core = core_with_three_users().await;
        core.handle(CoreCommand::ExecuteQuery {
            sql: "SELECT * FROM users".into(),
        })
        .await;

        let events = core.handle(CoreCommand::FetchTotalCount).await;

        assert_eq!(total_count_of(&events), Some(3));
    }

    /// With nothing to count, the answer is "no count", not an error: this is
    /// a background lookup, and a client that turned it into a toast would put
    /// a failure in front of the user for something they never asked for.
    #[tokio::test]
    async fn counting_with_no_active_query_reports_no_count() {
        let mut core = core_with_three_users().await;

        let events = core.handle(CoreCommand::FetchTotalCount).await;

        assert_eq!(total_count_of(&events), None);
        assert!(
            !events.iter().any(|e| matches!(e, CoreEvent::Error(_))),
            "{events:?}"
        );
    }

    /// GROUP BY / UNION / HAVING are still skipped — wrapping them in a
    /// `COUNT(*)` subquery re-runs the whole aggregation.
    #[tokio::test]
    async fn expensive_query_shapes_are_still_skipped() {
        let mut core = core_with_three_users().await;
        core.handle(CoreCommand::ExecuteQuery {
            sql: "SELECT name, COUNT(*) FROM users GROUP BY name".into(),
        })
        .await;

        let events = core.handle(CoreCommand::FetchTotalCount).await;

        assert_eq!(total_count_of(&events), None);
    }
}
