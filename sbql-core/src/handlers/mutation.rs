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
    let pool = match core.active_pool() {
        Ok(p) => p,
        Err(e) => return vec![CoreEvent::Error(e.to_string())],
    };
    match schema::execute_cell_update(&pool, &schema_name, &table, &pk_col, &pk_val, &target_col, &new_val).await {
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
    let pool = match core.active_pool() {
        Ok(p) => p,
        Err(e) => return vec![CoreEvent::Error(e.to_string())],
    };
    match schema::execute_row_delete(&pool, &schema_name, &table, &pk_col, &pk_val).await {
        Ok(()) => vec![CoreEvent::RowDeleted],
        Err(e) => vec![CoreEvent::Error(e.to_string())],
    }
}
