use uuid::Uuid;

use crate::{save_connections, ConnectionConfig, Core, CoreEvent, SbqlError};

pub(crate) async fn save(core: &mut Core, config: ConnectionConfig, password: Option<String>) -> Vec<CoreEvent> {
    if let Some(ref pw) = password {
        if let Err(e) = config.save_password(pw) {
            tracing::warn!("Keyring save failed (will use in-memory cache): {e}");
        }
        core.password_cache.insert(config.id, pw.clone());
    } else {
        if !core.password_cache.contains_key(&config.id) {
            if let Ok(pw) = config.load_password() {
                core.password_cache.insert(config.id, pw);
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
            .map(|pw| {
                core.password_cache.insert(id, pw.clone());
                pw
            })
            .or_else(|_| {
                Err(SbqlError::Keyring(format!(
                    "No password found for '{}'. Try re-entering it (e to edit).",
                    cfg.name
                )))
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
