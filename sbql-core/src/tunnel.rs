use std::collections::HashMap;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::ConnectionConfig;
use crate::error::{Result, SbqlError};

/// Holds active SSH tunnels keyed by connection ID.
pub struct TunnelManager {
    tunnels: Arc<RwLock<HashMap<Uuid, TunnelHandle>>>,
}

impl Default for TunnelManager {
    fn default() -> Self {
        Self::new()
    }
}

struct TunnelHandle {
    local_port: u16,
    shutdown: tokio::sync::watch::Sender<bool>,
}

impl TunnelManager {
    pub fn new() -> Self {
        Self {
            tunnels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Open an SSH tunnel for the given connection config.
    /// Returns the local port to connect through.
    pub async fn open(&self, config: &ConnectionConfig, ssh_password: &str) -> Result<u16> {
        if !config.ssh_enabled {
            return Err(SbqlError::SshTunnel("SSH not enabled".into()));
        }

        // Bind a random local port
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| SbqlError::SshTunnel(format!("Failed to bind local port: {e}")))?;
        let local_port = listener
            .local_addr()
            .map_err(|e| SbqlError::SshTunnel(e.to_string()))?
            .port();

        // Connect to SSH server
        let ssh_config = Arc::new(russh::client::Config::default());
        let mut handle = russh::client::connect(
            ssh_config,
            (config.ssh_host.as_str(), config.ssh_port),
            SshHandler,
        )
        .await
        .map_err(|e| SbqlError::SshTunnel(format!("SSH connect failed: {e}")))?;

        // Authenticate
        let authenticated = if config.ssh_auth_method == "key" {
            let key_path = config
                .ssh_key_path
                .as_deref()
                .ok_or_else(|| SbqlError::SshTunnel("SSH key path required".into()))?;
            let key = russh_keys::load_secret_key(key_path, None)
                .map_err(|e| SbqlError::SshTunnel(format!("Failed to load SSH key: {e}")))?;
            handle
                .authenticate_publickey(&config.ssh_user, Arc::new(key))
                .await
                .map_err(|e| SbqlError::SshTunnel(format!("SSH key auth failed: {e}")))?
        } else {
            handle
                .authenticate_password(&config.ssh_user, ssh_password)
                .await
                .map_err(|e| SbqlError::SshTunnel(format!("SSH password auth failed: {e}")))?
        };

        if !authenticated {
            return Err(SbqlError::SshTunnel("SSH authentication failed".into()));
        }

        let db_host = config.host.clone();
        let db_port = config.port;
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);

        // Spawn the forwarding loop.
        // `handle` stays in this task so we can open channels from it.
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept = listener.accept() => {
                        if let Ok((mut tcp_stream, _)) = accept {
                            // Open a direct-tcpip channel for each incoming connection
                            match handle.channel_open_direct_tcpip(
                                db_host.as_str(),
                                db_port as u32,
                                "127.0.0.1",
                                0,
                            ).await {
                                Ok(channel) => {
                                    let mut channel_stream = channel.into_stream();
                                    tokio::spawn(async move {
                                        let _ = tokio::io::copy_bidirectional(
                                            &mut tcp_stream,
                                            &mut channel_stream,
                                        ).await;
                                    });
                                }
                                Err(e) => {
                                    tracing::debug!("SSH channel open failed: {e}");
                                }
                            }
                        }
                    }
                    _ = shutdown_rx.changed() => break,
                }
            }
        });

        self.tunnels.write().await.insert(
            config.id,
            TunnelHandle {
                local_port,
                shutdown: shutdown_tx,
            },
        );

        Ok(local_port)
    }

    /// Close an SSH tunnel by connection ID.
    pub async fn close(&self, id: Uuid) {
        if let Some(handle) = self.tunnels.write().await.remove(&id) {
            let _ = handle.shutdown.send(true);
        }
    }

    /// Get the local port for an active tunnel.
    #[allow(dead_code)]
    pub async fn local_port(&self, id: Uuid) -> Option<u16> {
        self.tunnels.read().await.get(&id).map(|h| h.local_port)
    }
}

/// Minimal SSH client handler required by russh.
struct SshHandler;

#[async_trait::async_trait]
impl russh::client::Handler for SshHandler {
    type Error = SbqlError;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh_keys::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        // Accept all server keys (equivalent to StrictHostKeyChecking=no).
        // A future improvement could verify against known_hosts.
        Ok(true)
    }
}
