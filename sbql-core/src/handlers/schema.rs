use crate::{schema, Core, CoreEvent};

pub(crate) async fn list_tables(core: &mut Core) -> Vec<CoreEvent> {
    let pool = match core.active_pool().await {
        Ok(p) => p,
        Err(e) => return vec![CoreEvent::Error(e.to_string())],
    };
    match schema::list_tables(&pool).await {
        Ok(tables) => vec![CoreEvent::TableList(tables)],
        Err(e) => vec![CoreEvent::Error(e.to_string())],
    }
}

pub(crate) async fn get_primary_keys(
    core: &mut Core,
    schema_name: String,
    table: String,
) -> Vec<CoreEvent> {
    let pool = match core.active_pool().await {
        Ok(p) => p,
        Err(e) => return vec![CoreEvent::Error(e.to_string())],
    };
    match schema::get_primary_keys(&pool, &schema_name, &table).await {
        Ok(columns) => vec![CoreEvent::PrimaryKeys {
            schema: schema_name,
            table,
            columns,
        }],
        Err(e) => vec![CoreEvent::Error(e.to_string())],
    }
}

pub(crate) async fn load_diagram(core: &mut Core) -> Vec<CoreEvent> {
    let pool = match core.active_pool().await {
        Ok(p) => p,
        Err(e) => return vec![CoreEvent::Error(e.to_string())],
    };
    match schema::load_diagram(&pool).await {
        Ok(data) => vec![CoreEvent::DiagramLoaded(data)],
        Err(e) => vec![CoreEvent::Error(e.to_string())],
    }
}
