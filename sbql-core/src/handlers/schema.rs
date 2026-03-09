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

#[cfg(test)]
mod tests {
    use crate::{Core, CoreCommand, CoreEvent};

    #[tokio::test]
    async fn test_list_tables_no_connection() {
        let mut core = Core::default();
        let events = core.handle(CoreCommand::ListTables).await;
        assert!(
            matches!(&events[0], CoreEvent::Error(msg) if msg.contains("No active connection"))
        );
    }

    #[tokio::test]
    async fn test_get_primary_keys_no_connection() {
        let mut core = Core::default();
        let events = core
            .handle(CoreCommand::GetPrimaryKeys {
                schema: "public".into(),
                table: "users".into(),
            })
            .await;
        assert!(
            matches!(&events[0], CoreEvent::Error(msg) if msg.contains("No active connection"))
        );
    }

    #[tokio::test]
    async fn test_load_diagram_no_connection() {
        let mut core = Core::default();
        let events = core.handle(CoreCommand::LoadDiagram).await;
        assert!(
            matches!(&events[0], CoreEvent::Error(msg) if msg.contains("No active connection"))
        );
    }
}
