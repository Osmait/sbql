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
