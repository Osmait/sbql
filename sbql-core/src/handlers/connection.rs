use uuid::Uuid;

use crate::{save_connections, ConnectionConfig, Core, CoreError, CoreEvent, ErrorKind, SbqlError};

pub(crate) async fn save(
    core: &mut Core,
    config: ConnectionConfig,
    password: Option<String>,
) -> Vec<CoreEvent> {
    // Last gate before anything is persisted. Clients validate as the user
    // types, but they are not trusted to — a client that forgets (as the macOS
    // app did) would otherwise write a connection that can never open.
    if let Err(e) = config.validate() {
        return vec![CoreEvent::Error(CoreError::new(
            ErrorKind::Config,
            e.to_string(),
        ))];
    }

    // A keyring that refuses the write is not fatal — the connection is still
    // saved and the password is cached for this session — but the user has to
    // hear about it, otherwise the password is quietly gone on the next launch.
    let mut warning = None;
    if let Some(ref pw) = password {
        if let Err(e) = config.save_password(pw) {
            tracing::warn!("Keyring save failed (will use in-memory cache): {e}");
            // Reported as a *warning*, not an error: the connection was saved
            // and the password works for this session. Sent as a plain error it
            // was painted like a lost write, which is the opposite of true.
            //
            // The summary stays one line for a status bar; the store's own
            // complaint (which carries the fix-it hint) rides along in `detail`.
            warning = Some(
                CoreError::warning(
                    ErrorKind::Credentials,
                    "Connection saved, password NOT stored (session only)",
                )
                .with_detail(match &e {
                    SbqlError::Keyring(msg) => msg.clone(),
                    other => other.to_string(),
                }),
            );
        }
        core.password_cache.insert(config.id, pw.clone());
    } else {
        if let std::collections::hash_map::Entry::Vacant(e) = core.password_cache.entry(config.id) {
            if let Ok(pw) = config.load_password() {
                e.insert(pw);
            }
        }
    }

    // Editing a connection can leave a live pool built from the old settings:
    // connect_with_password reuses any pool keyed by this id, so without this
    // the user "reconnects" to the edited target while every query keeps
    // hitting the old host/database. If the target changed, the stale pool
    // (and any SSH tunnel) has to go, and the client has to hear it is now
    // disconnected.
    let mut dropped_stale_pool = false;
    let mut sort_dropped = None;
    if let Some(pos) = core.connections.iter().position(|c| c.id == config.id) {
        let target_changed = !core.connections[pos].same_target(&config);
        core.connections[pos] = config.clone();
        if target_changed && core.manager.active_ids().await.contains(&config.id) {
            core.manager.disconnect(config.id).await;
            if core.active_connection == Some(config.id) {
                core.active_connection = None;
                sort_dropped = core.reset_query_state();
            }
            dropped_stale_pool = true;
        }
    } else {
        core.connections.push(config.clone());
    }

    if let Err(e) = save_connections(&core.connections) {
        return vec![CoreEvent::error(&e)];
    }

    // The list stays first so existing consumers keep seeing it at index 0; the
    // warning is applied afterwards and is what the user ends up reading.
    let mut events = vec![CoreEvent::ConnectionList(core.connections.clone())];
    if dropped_stale_pool {
        events.push(CoreEvent::Disconnected(config.id));
    }
    // The query state went with the stale pool, sort included. Without this
    // the client keeps showing a sort indicator for an ORDER BY that no longer
    // exists in any query.
    events.extend(sort_dropped);
    if let Some(warning) = warning {
        events.push(CoreEvent::Error(warning));
    }
    events
}

pub(crate) async fn delete(core: &mut Core, id: Uuid) -> Vec<CoreEvent> {
    if let Some(pos) = core.connections.iter().position(|c| c.id == id) {
        let cfg = core.connections.remove(pos);
        let _ = cfg.delete_password();
        core.manager.disconnect(id).await;
    }
    if let Err(e) = save_connections(&core.connections) {
        return vec![CoreEvent::error(&e)];
    }
    vec![CoreEvent::ConnectionList(core.connections.clone())]
}

/// Ask Docker what is running and offer it as session-only connections.
///
/// Nothing is persisted and nothing already saved is disturbed: the result
/// replaces the previous scan's offer, and each connection's password goes
/// straight into the session cache so selecting one just connects.
pub(crate) async fn discover(core: &mut Core, dir: std::path::PathBuf) -> Vec<CoreEvent> {
    let found = crate::discovery::discover(&dir).await;

    // A container the user is already connected to keeps its pool: ids are
    // derived from the container id, so a re-scan re-identifies it rather than
    // orphaning the connection.
    core.discovered = found
        .into_iter()
        .map(|d| {
            core.password_cache
                .insert(d.connection.config.id, d.password);
            d.connection
        })
        .collect();

    tracing::info!("discovered {} database(s) in Docker", core.discovered.len());
    vec![CoreEvent::DiscoveredConnections(core.discovered.clone())]
}

/// Promote a discovered connection to a saved one.
///
/// Goes through the normal save path so the password reaches the keyring and
/// the config reaches `connections.toml` — the one moment scraped credentials
/// are written anywhere, and only because the user asked.
pub(crate) async fn save_discovered(core: &mut Core, id: Uuid) -> Vec<CoreEvent> {
    let Some(found) = core.discovered.iter().find(|d| d.config.id == id).cloned() else {
        return vec![CoreEvent::error(SbqlError::ConnectionNotFound(
            id.to_string(),
        ))];
    };
    if core.connections.iter().any(|c| c.id == id) {
        return vec![CoreEvent::error(CoreError::new(
            ErrorKind::Config,
            format!("'{}' is already saved", found.config.name),
        ))];
    }

    let password = core.password_cache.get(&id).cloned();
    let mut events = save(core, found.config.clone(), password).await;
    // It is a saved connection now, so it must not also be offered as a
    // discovery — the list would show it twice, under two different rules.
    core.discovered.retain(|d| d.config.id != id);
    events.push(CoreEvent::DiscoveredConnections(core.discovered.clone()));
    events
}

pub(crate) async fn connect(core: &mut Core, id: Uuid) -> Vec<CoreEvent> {
    // Saved connections first, then this session's discoveries: a discovered
    // one that has since been saved is the same id, and the saved copy is the
    // one the user chose to keep.
    let found = core.connections.iter().find(|c| c.id == id).or_else(|| {
        core.discovered
            .iter()
            .map(|d| &d.config)
            .find(|c| c.id == id)
    });
    let cfg = match found {
        Some(c) => c.clone(),
        None => {
            return vec![CoreEvent::error(SbqlError::ConnectionNotFound(
                id.to_string(),
            ))]
        }
    };

    let password = if let Some(pw) = core.password_cache.get(&id).cloned() {
        Ok(pw)
    } else {
        cfg.load_password()
            .inspect(|pw| {
                core.password_cache.insert(id, pw.clone());
            })
            // A missing entry is the user's to fix; anything else means the
            // credential store itself is unusable, and re-typing the password
            // would not help — so that cause has to survive.
            .map_err(|e| match e {
                SbqlError::PasswordNotFound(name) if crate::keyring_enabled() => {
                    SbqlError::Keyring(format!(
                        "No password found for '{name}'. Try re-entering it (e to edit)."
                    ))
                }
                // Keyring off by choice: nothing was ever stored, so say that
                // rather than implying something is broken.
                SbqlError::PasswordNotFound(name) => SbqlError::Keyring(format!(
                    "Keyring disabled — enter the password for '{name}' with 'e' (session only)."
                )),
                other => other,
            })
    };

    let password = match password {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Password lookup failed for '{}': {}", cfg.name, e);
            return vec![CoreEvent::error(&e)];
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
            vec![CoreEvent::error(&e)]
        }
    }
}

pub(crate) async fn disconnect(core: &mut Core, id: Uuid) -> Vec<CoreEvent> {
    core.manager.disconnect(id).await;
    let mut events = vec![CoreEvent::Disconnected(id)];
    if core.active_connection == Some(id) {
        core.active_connection = None;
        // Query state belongs to the connection that just closed. Left behind,
        // the next connection's first ApplyOrder/ApplyFilter/FetchPage would
        // build on the previous session's base_sql, columns and sort — running
        // the old query against the new database. The same goes for the
        // client's copy of the sort, which is why the reset hands one back.
        events.extend(core.reset_query_state());
    }
    events
}

#[cfg(test)]
mod tests {
    use crate::{
        config::CONFIG_DIR_ENV, ConnectionConfig, Core, CoreCommand, CoreEvent, ErrorKind,
    };
    use std::sync::OnceLock;

    /// Keep the test suite away from the developer's machine.
    ///
    /// Two separate hazards. Connections are persisted, so without an override
    /// these tests overwrite the real `~/.config/sbql/connections.toml`. And
    /// passwords go to the OS credential store, so on a desktop with a locked
    /// keyring every run pops up an unlock prompt — and leaves test credentials
    /// behind. Tests that care about the store opt back in explicitly.
    ///
    /// The temp dir is created once per test process and leaked, so it outlives
    /// every test that reads it back.
    fn isolate_from_the_machine() {
        static SCRATCH: OnceLock<tempfile::TempDir> = OnceLock::new();
        let dir = SCRATCH.get_or_init(|| tempfile::tempdir().expect("create temp config dir"));
        std::env::set_var(CONFIG_DIR_ENV, dir.path());
        std::env::set_var(crate::NO_KEYRING_ENV, "1");
    }

    #[tokio::test]
    async fn test_save_inserts_config_and_emits_list() {
        isolate_from_the_machine();
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
        isolate_from_the_machine();
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

    /// A client that skips its own validation must not be able to persist a
    /// connection that could never open. The macOS app only checked that the
    /// name was non-empty, so this is the gate that covers it.
    #[tokio::test]
    async fn test_save_rejects_a_config_that_would_never_connect() {
        isolate_from_the_machine();
        let mut core = Core::default();
        core.connections.clear();

        // Exactly what the macOS form allowed through: named, but no host.
        let mut config = ConnectionConfig::new_postgres("no-host", "", 5432, "u", "db");
        config.host = String::new();

        let events = core
            .handle(CoreCommand::SaveConnection {
                config,
                password: Some("pw".into()),
            })
            .await;

        match &events[0] {
            CoreEvent::Error(e) => {
                assert!(e.message.contains("Host is required"), "{e}");
                assert_eq!(e.kind, ErrorKind::Config);
            }
            other => panic!("expected a validation error, got {other:?}"),
        }
        assert!(
            core.connections.is_empty(),
            "an invalid connection must not be stored"
        );
    }

    /// Saving must survive an unusable credential store — common on Linux boxes
    /// with no Secret Service running. The connection is kept, the password
    /// stays usable for the session, and the user is told it was not persisted.
    ///
    /// The store is faulted rather than really broken, so this never touches
    /// the developer's keyring.
    #[tokio::test]
    async fn test_save_reports_unstorable_password_without_losing_connection() {
        isolate_from_the_machine();
        let _faulty_store = crate::config::store_fault::ForcedFailure::new();

        let mut core = Core::default();
        core.connections.clear();
        // Postgres (unlike SQLite) actually reaches for the keyring.
        let config = ConnectionConfig::new_postgres("test_no_store", "localhost", 5432, "u", "db");
        let id = config.id;

        let events = core
            .handle(CoreCommand::SaveConnection {
                config,
                password: Some("secret".into()),
            })
            .await;

        assert!(matches!(&events[0], CoreEvent::ConnectionList(list) if list
            .iter()
            .any(|c| c.id == id)));
        assert_eq!(core.password_cache.get(&id), Some(&"secret".to_string()));

        let warning = events
            .iter()
            .find_map(|e| match e {
                CoreEvent::Error(err) => Some(err.clone()),
                _ => None,
            })
            .expect("expected a warning when the keyring is unusable");

        // Reported as a warning, not a failure: the save itself worked, and a
        // client that paints this red is telling the user something false.
        assert!(warning.is_warning(), "{warning:?}");
        assert_eq!(warning.kind, ErrorKind::Credentials);
        assert!(warning.message.contains("password NOT stored"), "{warning}");
        // The summary has to fit a single-line status bar; anything longer
        // belongs in `detail`, which the client shows on request.
        assert!(
            warning.message.len() < 160,
            "summary too long ({}): {warning}",
            warning.message.len()
        );
        assert!(
            warning.detail.is_some(),
            "the store's own complaint should survive: {warning:?}"
        );
    }

    /// With the keyring switched off, the same save is silent: nothing was
    /// meant to be stored, so nothing is reported as wrong.
    #[tokio::test]
    async fn test_save_with_the_keyring_disabled_warns_about_nothing() {
        isolate_from_the_machine();
        let mut core = Core::default();
        core.connections.clear();
        let config = ConnectionConfig::new_postgres("no_keyring", "localhost", 5432, "u", "db");
        let id = config.id;

        let events = core
            .handle(CoreCommand::SaveConnection {
                config,
                password: Some("secret".into()),
            })
            .await;

        assert!(
            !events.iter().any(|e| matches!(e, CoreEvent::Error(_))),
            "opting out of the keyring is not an error: {events:?}"
        );
        assert_eq!(core.password_cache.get(&id), Some(&"secret".to_string()));
    }

    #[tokio::test]
    async fn test_delete_removes_connection() {
        isolate_from_the_machine();
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

    /// A discovered connection is not in `connections`, but selecting it has
    /// to just work — that is the whole point of offering it.
    #[tokio::test]
    async fn test_connect_uses_a_discovered_connection() {
        isolate_from_the_machine();
        let mut core = Core::default();
        let config = ConnectionConfig::new_sqlite("from_docker", ":memory:");
        let id = config.id;
        core.discovered.push(sbql_discovery(config));
        core.password_cache.insert(id, String::new());

        let events = core.handle(CoreCommand::Connect(id)).await;

        assert!(matches!(&events[0], CoreEvent::Connected(cid) if *cid == id));
        assert!(
            core.connections.is_empty(),
            "connecting must not persist a discovered connection"
        );
    }

    /// Saving is the one moment scraped credentials are written anywhere, and
    /// afterwards the connection must be offered once, not twice.
    #[tokio::test]
    async fn test_save_discovered_persists_it_and_stops_offering_it() {
        isolate_from_the_machine();
        let mut core = Core::default();
        core.connections.clear();
        let config = ConnectionConfig::new_sqlite("from_docker", ":memory:");
        let id = config.id;
        core.discovered.push(sbql_discovery(config));
        core.password_cache.insert(id, "scraped".into());

        let events = core.handle(CoreCommand::SaveDiscovered(id)).await;

        assert!(
            core.connections.iter().any(|c| c.id == id),
            "it should now be a saved connection"
        );
        assert!(
            core.discovered.is_empty(),
            "and no longer offered as a discovery"
        );
        assert!(events
            .iter()
            .any(|e| matches!(e, CoreEvent::DiscoveredConnections(d) if d.is_empty())));
        // The password survives the promotion, or the saved connection could
        // never open.
        assert_eq!(
            core.password_cache.get(&id).map(String::as_str),
            Some("scraped")
        );
    }

    #[tokio::test]
    async fn test_save_discovered_rejects_an_unknown_id() {
        isolate_from_the_machine();
        let mut core = Core::default();
        let events = core
            .handle(CoreCommand::SaveDiscovered(uuid::Uuid::new_v4()))
            .await;
        assert!(matches!(&events[0], CoreEvent::Error(_)));
    }

    /// Wrap a config the way discovery would.
    fn sbql_discovery(config: ConnectionConfig) -> crate::DiscoveredConnection {
        crate::DiscoveredConnection {
            config,
            source: crate::DiscoverySource::Container {
                name: "test-container".into(),
            },
        }
    }

    /// Editing where a connection points must drop the live pool built from
    /// the old settings — otherwise the next Connect reuses it and every query
    /// runs against the pre-edit host/database while the UI claims otherwise.
    #[tokio::test]
    async fn test_editing_a_connected_target_drops_the_stale_pool() {
        isolate_from_the_machine();
        let mut core = Core::default();
        core.connections.clear();
        let config = crate::ConnectionConfig::new_sqlite("editable", ":memory:");
        let id = config.id;
        core.connections.push(config.clone());
        core.password_cache.insert(id, String::new());
        core.handle(CoreCommand::Connect(id)).await;
        assert_eq!(core.active_connection, Some(id));
        core.sort_state = Some(("name".into(), crate::SortDirection::Ascending));

        let mut edited = config.clone();
        edited.file_path = Some("/somewhere/else.db".into());
        let events = core
            .handle(CoreCommand::SaveConnection {
                config: edited,
                password: None,
            })
            .await;

        assert!(
            events
                .iter()
                .any(|e| matches!(e, CoreEvent::Disconnected(did) if *did == id)),
            "client must be told the edited connection is no longer live: {events:?}"
        );
        assert!(core.active_connection.is_none());
        assert!(
            core.manager.get(id).await.is_err(),
            "stale pool must be gone"
        );
        // The session's sort went with the pool, so the client's copy has to go
        // too — otherwise the header keeps an arrow for a query that is gone.
        assert!(core.sort_state.is_none());
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CoreEvent::SortChanged(None))),
            "{events:?}"
        );
    }

    /// A rename does not change where the connection points, so the live pool
    /// (and the user's session) survives the save.
    #[tokio::test]
    async fn test_renaming_a_connected_target_keeps_the_pool() {
        isolate_from_the_machine();
        let mut core = Core::default();
        core.connections.clear();
        let config = crate::ConnectionConfig::new_sqlite("old-name", ":memory:");
        let id = config.id;
        core.connections.push(config.clone());
        core.password_cache.insert(id, String::new());
        core.handle(CoreCommand::Connect(id)).await;

        let mut renamed = config.clone();
        renamed.name = "new-name".into();
        let events = core
            .handle(CoreCommand::SaveConnection {
                config: renamed,
                password: None,
            })
            .await;

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, CoreEvent::Disconnected(_))),
            "a rename must not tear down the session: {events:?}"
        );
        assert_eq!(core.active_connection, Some(id));
        assert!(core.manager.get(id).await.is_ok());
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
