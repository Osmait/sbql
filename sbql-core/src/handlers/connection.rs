use uuid::Uuid;

use crate::{save_connections, ConnectionConfig, Core, CoreEvent, SbqlError};

pub(crate) async fn save(
    core: &mut Core,
    config: ConnectionConfig,
    password: Option<String>,
) -> Vec<CoreEvent> {
    if let Some(ref pw) = password {
        if let Err(e) = config.save_password(pw) {
            tracing::warn!("Keyring save failed (will use in-memory cache): {e}");
        }
        core.password_cache.insert(config.id, pw.clone());
    } else {
        if let std::collections::hash_map::Entry::Vacant(e) = core.password_cache.entry(config.id) {
            if let Ok(pw) = config.load_password() {
                e.insert(pw);
            }
        }
    }

    if let Some(pos) = core.connections.iter().position(|c| c.id == config.id) {
        core.connections[pos] = config;
    } else {
        core.connections.push(config);
    }

    if let Err(e) = save_connections(&core.connections) {
        return vec![CoreEvent::Error(e.to_string())];
    }
    vec![CoreEvent::ConnectionList(core.connections.clone())]
}

pub(crate) async fn delete(core: &mut Core, id: Uuid) -> Vec<CoreEvent> {
    if let Some(pos) = core.connections.iter().position(|c| c.id == id) {
        let cfg = core.connections.remove(pos);
        let _ = cfg.delete_password();
        core.manager.disconnect(id).await;
    }
    if let Err(e) = save_connections(&core.connections) {
        return vec![CoreEvent::Error(e.to_string())];
    }
    vec![CoreEvent::ConnectionList(core.connections.clone())]
}

pub(crate) async fn connect(core: &mut Core, id: Uuid) -> Vec<CoreEvent> {
    let cfg = match core.connections.iter().find(|c| c.id == id) {
        Some(c) => c.clone(),
        None => return vec![CoreEvent::Error(format!("Connection {} not found", id))],
    };

    let password = if let Some(pw) = core.password_cache.get(&id).cloned() {
        Ok(pw)
    } else {
        cfg.load_password()
            .inspect(|pw| {
                core.password_cache.insert(id, pw.clone());
            })
            .map_err(|_| {
                SbqlError::Keyring(format!(
                    "No password found for '{}'. Try re-entering it (e to edit).",
                    cfg.name
                ))
            })
    };

    let password = match password {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Password lookup failed for '{}': {}", cfg.name, e);
            return vec![CoreEvent::Error(e.to_string())];
        }
    };

    match core.manager.connect_with_password(&cfg, &password).await {
        Ok(()) => {
            core.active_connection = Some(id);
            tracing::info!("Connected: {}", cfg.name);
            vec![CoreEvent::Connected(id)]
        }
        Err(e) => {
            tracing::error!("Connect failed for '{}': {}", cfg.name, e);
            vec![CoreEvent::Error(e.to_string())]
        }
    }
}

pub(crate) async fn disconnect(core: &mut Core, id: Uuid) -> Vec<CoreEvent> {
    core.manager.disconnect(id).await;
    if core.active_connection == Some(id) {
        core.active_connection = None;
    }
    vec![CoreEvent::Disconnected(id)]
}

#[cfg(test)]
mod tests {
    use crate::{ConnectionConfig, Core, CoreCommand, CoreEvent};

    #[tokio::test]
    async fn test_save_inserts_config_and_emits_list() {
        let mut core = Core::default();
        core.connections.clear();
        let config = ConnectionConfig::new_sqlite("test_save", ":memory:");
        let id = config.id;

        let events = core
            .handle(CoreCommand::SaveConnection {
                config,
                password: None,
            })
            .await;

        match &events[0] {
            CoreEvent::ConnectionList(list) => {
                assert!(list.iter().any(|c| c.id == id));
            }
            CoreEvent::Error(msg) => {
                // save_connections may fail if config dir is not writable in CI,
                // that's acceptable - we verify the in-memory state instead.
                panic!("Unexpected error: {msg}");
            }
            _ => panic!("Expected ConnectionList"),
        }
    }

    #[tokio::test]
    async fn test_save_with_password_caches() {
        let mut core = Core::default();
        core.connections.clear();
        let config = ConnectionConfig::new_sqlite("test_pw", ":memory:");
        let id = config.id;

        let _events = core
            .handle(CoreCommand::SaveConnection {
                config,
                password: Some("secret".into()),
            })
            .await;

        assert_eq!(core.password_cache.get(&id), Some(&"secret".to_string()));
    }

    #[tokio::test]
    async fn test_delete_removes_connection() {
        let mut core = Core::default();
        core.connections.clear();
        let config = ConnectionConfig::new_sqlite("to_delete", ":memory:");
        let id = config.id;
        core.connections.push(config);

        let events = core.handle(CoreCommand::DeleteConnection(id)).await;
        if let CoreEvent::ConnectionList(list) = &events[0] {
            assert!(!list.iter().any(|c| c.id == id));
        }
    }

    #[tokio::test]
    async fn test_connect_sqlite_emits_connected() {
        let mut core = Core::default();
        let config = ConnectionConfig::new_sqlite("test_conn", ":memory:");
        let id = config.id;
        core.connections.push(config);
        core.password_cache.insert(id, String::new());

        let events = core.handle(CoreCommand::Connect(id)).await;
        assert!(matches!(&events[0], CoreEvent::Connected(cid) if *cid == id));
        assert_eq!(core.active_connection, Some(id));
    }

    #[tokio::test]
    async fn test_disconnect_clears_active_connection() {
        let mut core = Core::default();
        let config = ConnectionConfig::new_sqlite("test_dc", ":memory:");
        let id = config.id;
        core.connections.push(config);
        core.password_cache.insert(id, String::new());
        core.handle(CoreCommand::Connect(id)).await;
        assert_eq!(core.active_connection, Some(id));

        let events = core.handle(CoreCommand::Disconnect(id)).await;
        assert!(matches!(&events[0], CoreEvent::Disconnected(did) if *did == id));
        assert!(core.active_connection.is_none());
    }
}
