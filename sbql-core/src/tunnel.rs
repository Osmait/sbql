//! SSH tunnels, for databases that are only reachable through a bastion.
//!
//! A tunnel binds a random loopback port, forwards it over an authenticated
//! SSH connection to the real host, and the pool is then built against
//! `127.0.0.1:<that port>` instead. [`ConnectionManager`](crate::connection)
//! opens and closes them; nothing else needs to know a tunnel is involved.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::ConnectionConfig;
use crate::error::{Result, SbqlError};

/// Holds active SSH tunnels keyed by connection ID.
#[derive(Debug)]
pub(crate) struct TunnelManager {
    tunnels: Arc<RwLock<HashMap<Uuid, TunnelHandle>>>,
}

impl Default for TunnelManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct TunnelHandle {
    local_port: u16,
    shutdown: tokio::sync::watch::Sender<bool>,
}

impl TunnelManager {
    pub(crate) fn new() -> Self {
        Self {
            tunnels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Open an SSH tunnel for the given connection config.
    /// Returns the local port to connect through.
    pub(crate) async fn open(&self, config: &ConnectionConfig, ssh_password: &str) -> Result<u16> {
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
            SshHandler {
                host: config.ssh_host.clone(),
                port: config.ssh_port,
            },
        )
        .await
        .map_err(|e| SbqlError::SshTunnel(format!("SSH connect failed: {e}")))?;

        // Authenticate.
        //
        // These used to return `bool`. They now return `AuthResult`, which
        // distinguishes outright rejection from a partial success that wants
        // a second factor — `success()` is true only for a completed auth, so
        // a partial success still fails closed here.
        let auth_result = if config.ssh_auth_method == "key" {
            let key_path = config
                .ssh_key_path
                .as_deref()
                .ok_or_else(|| SbqlError::SshTunnel("SSH key path required".into()))?;
            let key = russh::keys::load_secret_key(key_path, None)
                .map_err(|e| SbqlError::SshTunnel(format!("Failed to load SSH key: {e}")))?;
            // An RSA key has to be signed under a hash the server actually
            // advertises. russh defaults `None` to ssh-rsa (SHA-1), which
            // OpenSSH 8.8+ refuses by default — that reads to the user as
            // "wrong key", not "wrong signature algorithm". Asking the server
            // first picks rsa-sha2-512/256 where offered. Ignored for every
            // other key type.
            let rsa_hash = handle
                .best_supported_rsa_hash()
                .await
                .map_err(|e| SbqlError::SshTunnel(format!("SSH key auth failed: {e}")))?
                .flatten();
            handle
                .authenticate_publickey(
                    &config.ssh_user,
                    russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key), rsa_hash),
                )
                .await
                .map_err(|e| SbqlError::SshTunnel(format!("SSH key auth failed: {e}")))?
        } else {
            handle
                .authenticate_password(&config.ssh_user, ssh_password)
                .await
                .map_err(|e| SbqlError::SshTunnel(format!("SSH password auth failed: {e}")))?
        };

        if !auth_result.success() {
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
                                        // A forwarding failure has no caller left
                                        // to return to, but it must not vanish
                                        // either: a tunnel that quietly drops
                                        // connections reads to the user as the
                                        // database being flaky.
                                        if let Err(e) = tokio::io::copy_bidirectional(
                                            &mut tcp_stream,
                                            &mut channel_stream,
                                        ).await {
                                            tracing::debug!("SSH tunnel forwarding ended: {e}");
                                        }
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
    pub(crate) async fn close(&self, id: Uuid) {
        if let Some(handle) = self.tunnels.write().await.remove(&id) {
            let _ = handle.shutdown.send(true);
        }
    }

    /// Get the local port for an active tunnel.
    #[allow(dead_code)]
    pub(crate) async fn local_port(&self, id: Uuid) -> Option<u16> {
        self.tunnels.read().await.get(&id).map(|h| h.local_port)
    }
}

/// SSH client handler: verifies the server's host key against the user's
/// `known_hosts` file.
struct SshHandler {
    host: String,
    port: u16,
}

// No `#[async_trait]`: from russh 0.5x the `Handler` methods return
// `impl Future` natively, so a plain `async fn` implements them.
impl russh::client::Handler for SshHandler {
    type Error = SbqlError;

    /// Same policy as OpenSSH's `StrictHostKeyChecking=accept-new`: a host
    /// already in `~/.ssh/known_hosts` must present the recorded key, an
    /// unknown host is trusted on first use and recorded, and a *changed* key
    /// is refused outright — accepting everything silently, as this handler
    /// once did, hands the SSH password and all database traffic to whoever
    /// answers on the wire.
    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        match russh::keys::check_known_hosts(&self.host, self.port, server_public_key) {
            Ok(true) => Ok(true),
            Ok(false) => {
                russh::keys::known_hosts::learn_known_hosts(
                    &self.host,
                    self.port,
                    server_public_key,
                )
                .map_err(|e| {
                    SbqlError::SshTunnel(format!(
                        "Could not record host key for {}:{} in known_hosts: {e}",
                        self.host, self.port
                    ))
                })?;
                tracing::info!(
                    "Trusting {}:{} on first use; host key {} recorded in known_hosts",
                    self.host,
                    self.port,
                    server_public_key.fingerprint(russh::keys::HashAlg::Sha256)
                );
                Ok(true)
            }
            Err(russh::keys::Error::KeyChanged { line }) => Err(SbqlError::SshTunnel(format!(
                "HOST KEY CHANGED for {}:{} — offered key {} does not match known_hosts line \
                 {line}. This can mean a man-in-the-middle attack; connection refused. If the \
                 server key really changed, remove that line and reconnect.",
                self.host,
                self.port,
                server_public_key.fingerprint(russh::keys::HashAlg::Sha256)
            ))),
            Err(e) => Err(SbqlError::SshTunnel(format!(
                "Could not verify host key for {}:{}: {e}",
                self.host, self.port
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;
    use crate::config::ConnectionConfig;

    /// Where the test's throwaway `known_hosts`, host keys and client key live.
    ///
    /// Also the thing that makes the test opt-in: without it we would be
    /// learning and then deliberately invalidating host keys in the
    /// developer's real `~/.ssh/known_hosts`.
    const LAB_VAR: &str = "SBQL_SSH_LAB";

    /// `russh::keys` resolves `known_hosts` from `$HOME` with no way to pass a
    /// path in, so the only way to redirect it is to run the test binary with
    /// `HOME` already pointing at the lab. Asserting that, rather than calling
    /// `set_var` ourselves, keeps the test from reaching into a process-global
    /// that the other tests in this binary also read.
    fn lab_dir() -> PathBuf {
        let lab = PathBuf::from(std::env::var(LAB_VAR).unwrap_or_else(|_| {
            panic!(
                "set {LAB_VAR} and HOME to the same throwaway directory before running this test"
            )
        }));
        let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
        assert_eq!(
            home, lab,
            "HOME must equal {LAB_VAR} — otherwise this test rewrites your real known_hosts"
        );
        lab
    }

    /// OpenSSH 9.8 split the server into `sshd` plus helpers, and a non-root
    /// `sshd` started by absolute path cannot find them on its own. The path
    /// differs per distribution.
    fn sshd_helper(name: &str) -> String {
        for dir in ["/usr/lib/ssh", "/usr/lib/openssh", "/usr/libexec/openssh"] {
            let c = format!("{dir}/{name}");
            if Path::new(&c).exists() {
                return c;
            }
        }
        panic!("no {name} helper found; is OpenSSH 9.8+ installed?");
    }

    fn run(cmd: &str, args: &[&str]) {
        let out = Command::new(cmd)
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("could not run {cmd}: {e}"));
        assert!(out.status.success(), "{cmd} {args:?} failed: {out:?}");
    }

    /// Two host keys and one client key. `hostkey_a` is what the server
    /// presents first; `hostkey_b` is the impersonator.
    fn build_lab(lab: &Path) {
        for name in ["hostkey_a", "hostkey_b", "client"] {
            let path = lab.join(name);
            if path.exists() {
                continue;
            }
            run(
                "ssh-keygen",
                &[
                    "-q",
                    "-t",
                    "ed25519",
                    "-N",
                    "",
                    "-C",
                    name,
                    "-f",
                    &path.display().to_string(),
                ],
            );
        }
        std::fs::copy(lab.join("client.pub"), lab.join("authorized_keys")).unwrap();
    }

    fn free_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    fn write_cfg(lab: &Path, which: &str, port: u16) -> PathBuf {
        let p = lab.join(format!("sshd_config_{which}"));
        let mut f = std::fs::File::create(&p).unwrap();
        write!(
            f,
            "ListenAddress 127.0.0.1\nPort {port}\nHostKey {lab}/hostkey_{which}\n\
             AuthorizedKeysFile {lab}/authorized_keys\nSshdSessionPath {sess}\n\
             SshdAuthPath {auth}\nUsePAM no\nPasswordAuthentication no\n\
             KbdInteractiveAuthentication no\nPubkeyAuthentication yes\n\
             AllowTcpForwarding yes\nPermitOpen any\nStrictModes no\n\
             PerSourcePenalties no\nLogLevel DEBUG1\n",
            lab = lab.display(),
            sess = sshd_helper("sshd-session"),
            auth = sshd_helper("sshd-auth"),
        )
        .unwrap();
        p
    }

    async fn start_sshd(lab: &Path, which: &str, port: u16) -> Child {
        let cfg = write_cfg(lab, which, port);
        let mut child = Command::new("/usr/sbin/sshd")
            .arg("-D")
            .arg("-f")
            .arg(&cfg)
            .arg("-E")
            .arg(lab.join(format!("sshd_{which}.log")))
            .spawn()
            .expect("could not start sshd");
        let mut up = false;
        for _ in 0..100 {
            if tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .is_ok()
            {
                up = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        if !up {
            // Reap it before failing; a leaked `sshd` would hold the port and
            // make every later run of this test fail for the wrong reason.
            stop(&mut child);
            panic!("sshd never accepted a connection on {port}");
        }
        child
    }

    fn stop(child: &mut Child) {
        let _unused = child.kill();
        let _unused = child.wait();
    }

    /// Stands in for the database behind the bastion: whatever the tunnel
    /// forwards to it comes straight back.
    async fn start_echo() -> u16 {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0u8; 64];
                    while let Ok(n) = s.read(&mut buf).await {
                        if n == 0 {
                            break;
                        }
                        let _unused = s.write_all(&buf[..n]).await;
                    }
                });
            }
        });
        port
    }

    async fn round_trip(port: u16) -> String {
        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        s.write_all(b"hello-through-the-tunnel").await.unwrap();
        let mut buf = [0u8; 64];
        let n = s.read(&mut buf).await.unwrap();
        String::from_utf8_lossy(&buf[..n]).into_owned()
    }

    /// The one branch in this crate that stands between a user and a
    /// man-in-the-middle, exercised against a real OpenSSH server.
    ///
    /// A `check_server_key` that returns `Ok(true)` unconditionally compiles,
    /// passes every other test in this repository, and silently hands the SSH
    /// password and all database traffic to whoever answers on the wire. Only
    /// a live handshake tells the two apart, which is why this test spawns an
    /// `sshd` rather than mocking one. Run it after any `russh` bump.
    ///
    /// `#[ignore]` because it needs OpenSSH 9.8+ installed, binds a port, and
    /// requires `HOME` to be redirected — see [`lab_dir`]:
    ///
    /// ```sh
    /// LAB=$(mktemp -d)
    /// HOME=$LAB SBQL_SSH_LAB=$LAB cargo test -p sbql-core --lib \
    ///     tunnel::tests -- --ignored --nocapture --test-threads=1
    /// ```
    ///
    /// Note that a non-root `sshd` can only authenticate the user who started
    /// it, so the tunnel logs in as `$USER` with a key we generate.
    #[tokio::test]
    #[ignore = "needs a local OpenSSH server; see the doc comment for how to run it"]
    async fn a_changed_host_key_is_refused_while_an_unknown_one_is_learned() {
        let lab = lab_dir();
        build_lab(&lab);
        let known = lab.join(".ssh/known_hosts");
        let _unused = std::fs::remove_file(&known);

        let ssh_port = free_port();
        let db_port = start_echo().await;
        let manager = TunnelManager::new();

        let mut config = ConnectionConfig::new_postgres("live", "127.0.0.1", db_port, "user", "db");
        config.ssh_enabled = true;
        config.ssh_host = "127.0.0.1".into();
        config.ssh_port = ssh_port;
        config.ssh_user = std::env::var("USER").unwrap();
        config.ssh_auth_method = "key".into();
        config.ssh_key_path = Some(lab.join("client").display().to_string());

        // --- an unknown host is trusted on first use, and recorded ---------
        let mut sshd = start_sshd(&lab, "a", ssh_port).await;
        let local = manager.open(&config, "").await.expect("tunnel should open");
        assert_eq!(round_trip(local).await, "hello-through-the-tunnel");
        let learned = std::fs::read_to_string(&known).expect("known_hosts should be written");
        assert!(learned.contains(&format!("[127.0.0.1]:{ssh_port}")));
        manager.close(config.id).await;

        // --- a host already in known_hosts reconnects, and is not re-learned
        let local = manager
            .open(&config, "")
            .await
            .expect("tunnel should reopen");
        assert_eq!(round_trip(local).await, "hello-through-the-tunnel");
        assert_eq!(
            learned,
            std::fs::read_to_string(&known).unwrap(),
            "a known host should not be appended a second time"
        );
        manager.close(config.id).await;

        // --- a *changed* host key is refused -------------------------------
        stop(&mut sshd);
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let mut impostor = start_sshd(&lab, "b", ssh_port).await;
        let err = manager
            .open(&config, "")
            .await
            .expect_err("a changed host key must not be accepted");
        stop(&mut impostor);

        let msg = err.to_string();
        assert!(msg.contains("HOST KEY CHANGED"), "{msg}");
        assert!(msg.contains("known_hosts line"), "{msg}");
        assert!(
            msg.contains("SHA256:"),
            "fingerprint should be reported: {msg}"
        );
        assert_eq!(
            learned,
            std::fs::read_to_string(&known).unwrap(),
            "a rejected key must not be written to known_hosts"
        );
    }
}
