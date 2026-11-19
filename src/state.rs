use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, OwnedMappedMutexGuard, OwnedMutexGuard};

pub use crate::common::AppConfig;
use crate::dbeaver::{self, ConnectParams};
use crate::pg::{self, ConnectOptions, Session};
use crate::pseudo::Pseudonymizer;
use crate::tools::errors::{ErrorCategory, ToolError};

pub struct AppState {
    pub config: AppConfig,
    pseudonymizer: Result<Arc<Pseudonymizer>, String>,
    sessions: Mutex<HashMap<String, SessionSlot>>,
    connect_deadline: Duration,
}

type SessionSlot = Arc<Mutex<Option<Session>>>;

pub type SessionGuard = OwnedMappedMutexGuard<Option<Session>, Session>;

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let pseudonymizer = Pseudonymizer::open_default(&config)
            .map(Arc::new)
            .map_err(|error| error.to_string());
        if let Err(error) = &pseudonymizer {
            tracing::error!(%error, "pseudonym store unavailable; data tools will refuse to answer");
        }
        Self::with_pseudonymizer(config, pseudonymizer)
    }

    pub fn with_pseudonymizer(
        config: AppConfig,
        pseudonymizer: Result<Arc<Pseudonymizer>, String>,
    ) -> Self {
        Self {
            config,
            pseudonymizer,
            sessions: Mutex::new(HashMap::new()),
            connect_deadline: pg::CONNECT_DEADLINE,
        }
    }

    pub fn pseudonymizer(&self) -> Result<Arc<Pseudonymizer>, ToolError> {
        self.pseudonymizer.clone().map_err(|error| {
            ToolError::new(
                ErrorCategory::WithheldFailClosed,
                "pseudonym_store_unavailable",
                error,
            )
            .with_hint("ask the developer to run pg-pseudo-mcp outside the agent to repair the pseudonym store")
        })
    }

    pub async fn session(&self, connection: &str) -> Result<SessionGuard, ToolError> {
        let params = dbeaver::find_connection(&self.config, connection)?;
        let options = connect_options(&self.config, &params);
        let slot = self.slot(connection).await;
        let mut cached = slot.lock_owned().await;
        let session = match cached.take() {
            Some(session) if session.options() == &options => session,
            _ => pg::connect_within(&options, self.connect_deadline)
                .await
                .map_err(|error| {
                    ToolError::new(
                        ErrorCategory::ConnectionUnavailable,
                        "connection_failed",
                        format!("could not connect to {connection}: {error}"),
                    )
                    .with_hint("check that the database is reachable from this machine, then retry")
                    .retryable()
                })?,
        };
        Ok(OwnedMutexGuard::map(cached, |slot| slot.insert(session)))
    }

    async fn slot(&self, connection: &str) -> SessionSlot {
        self.sessions
            .lock()
            .await
            .entry(connection.to_string())
            .or_default()
            .clone()
    }

    #[cfg(test)]
    fn cache_for_tests(&self, connection: &str, session: Session) {
        let slot = self
            .sessions
            .try_lock()
            .expect("idle state")
            .entry(connection.to_string())
            .or_default()
            .clone();
        *slot.try_lock().expect("idle slot") = Some(session);
    }
}

pub fn connect_options(config: &AppConfig, params: &ConnectParams) -> ConnectOptions {
    ConnectOptions {
        host: params.host.clone(),
        port: params.port,
        database: params.database.clone(),
        user: params.user.clone(),
        password: params.password.expose().to_string(),
        sslmode: match params.sslmode {
            dbeaver::SslMode::Disable => pg::SslMode::Disable,
            dbeaver::SslMode::Allow => pg::SslMode::Allow,
            dbeaver::SslMode::Prefer => pg::SslMode::Prefer,
            dbeaver::SslMode::Require => pg::SslMode::Require,
            dbeaver::SslMode::VerifyCa => pg::SslMode::VerifyCa,
            dbeaver::SslMode::VerifyFull => pg::SslMode::VerifyFull,
        },
        tunnel: params.tunnel.as_ref().map(|tunnel| pg::Tunnel {
            host: tunnel.host.clone(),
            port: tunnel.port,
            user: tunnel.user.clone(),
            auth: match &tunnel.auth {
                dbeaver::SshAuth::PublicKey {
                    key_path,
                    passphrase,
                } => pg::TunnelAuth::PublicKey {
                    key_path: key_path.clone(),
                    passphrase: passphrase
                        .as_ref()
                        .map(|passphrase| passphrase.expose().to_string()),
                },
                dbeaver::SshAuth::Password(password) => {
                    pg::TunnelAuth::Password(password.expose().to_string())
                }
                dbeaver::SshAuth::Agent => pg::TunnelAuth::Agent,
            },
            remote_host: tunnel.remote_host.clone(),
            remote_port: tunnel.remote_port,
            verify_host_key: tunnel.verify_host_key,
            known_hosts: config.ssh_known_hosts.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use serde_json::{Map, Value, json};
    use tokio::net::TcpListener;

    use super::*;
    use crate::dbeaver::credentials::encrypt_for_tests;

    struct Endpoint {
        name: &'static str,
        port: u16,
        password: &'static str,
    }

    fn write_workspace(root: &Path, endpoints: &[Endpoint]) {
        let dir = root.join("workspace6").join("General").join(".dbeaver");
        std::fs::create_dir_all(&dir).expect("workspace dir");
        let mut connections = Map::new();
        let mut credentials = Map::new();
        for endpoint in endpoints {
            let id = format!("postgres-jdbc-{}", endpoint.name);
            connections.insert(
                id.clone(),
                json!({
                    "provider": "postgresql",
                    "driver": "postgres-jdbc",
                    "name": endpoint.name,
                    "save-password": true,
                    "configuration": {
                        "host": "127.0.0.1",
                        "port": endpoint.port.to_string(),
                        "database": "app",
                        "type": "dev",
                        "auth-model": "native",
                        "provider-properties": { "sslmode": "disable" }
                    }
                }),
            );
            credentials.insert(
                id,
                json!({ "#connection": { "user": "reader", "password": endpoint.password } }),
            );
        }
        let sources = json!({ "folders": {}, "connections": Value::Object(connections) });
        std::fs::write(dir.join("data-sources.json"), sources.to_string()).expect("sources");
        std::fs::write(
            dir.join("credentials-config.json"),
            encrypt_for_tests(&Value::Object(credentials).to_string(), [7u8; 16]),
        )
        .expect("credentials");
    }

    fn state_for(root: &Path) -> AppState {
        AppState::with_pseudonymizer(
            AppConfig {
                dbeaver_home: Some(root.to_path_buf()),
                ..AppConfig::default()
            },
            Err("unused".into()),
        )
    }

    async fn stalled_port() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("address").port();
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((socket, _)) = listener.accept().await {
                held.push(socket);
            }
        });
        port
    }

    async fn refused_port() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        listener.local_addr().expect("address").port()
    }

    #[tokio::test]
    async fn a_server_stalling_after_the_socket_opens_neither_hangs_nor_blocks_other_connections() {
        let root = tempfile::tempdir().expect("dbeaver dir");
        write_workspace(
            root.path(),
            &[
                Endpoint {
                    name: "stalled",
                    port: stalled_port().await,
                    password: "p",
                },
                Endpoint {
                    name: "refused",
                    port: refused_port().await,
                    password: "p",
                },
            ],
        );
        let mut state = state_for(root.path());
        state.connect_deadline = Duration::from_millis(500);
        let state = Arc::new(state);
        let started = Instant::now();
        let stalled = tokio::spawn({
            let state = state.clone();
            async move { state.session("stalled").await.map(|_| ()) }
        });
        tokio::time::sleep(Duration::from_millis(100)).await;
        let other = tokio::time::timeout(Duration::from_secs(1), state.session("refused"))
            .await
            .expect("another connection is not blocked by the stalled one");
        assert!(other.is_err());
        let stalled = tokio::time::timeout(Duration::from_secs(3), stalled)
            .await
            .expect("the stalled connection gives up within its bound")
            .expect("task");
        let error = stalled.expect_err("stalled connection fails");
        assert!(error.message.contains("timed out"), "{}", error.message);
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[tokio::test]
    async fn a_changed_password_in_dbeaver_replaces_the_cached_session() {
        let root = tempfile::tempdir().expect("dbeaver dir");
        let port = refused_port().await;
        write_workspace(
            root.path(),
            &[Endpoint {
                name: "app",
                port,
                password: "old",
            }],
        );
        let state = state_for(root.path());
        let params = dbeaver::find_connection(&state.config, "app").expect("params");
        state.cache_for_tests(
            "app",
            Session::idle(connect_options(&state.config, &params)),
        );
        assert!(state.session("app").await.is_ok());
        write_workspace(
            root.path(),
            &[Endpoint {
                name: "app",
                port,
                password: "new",
            }],
        );
        let error = state
            .session("app")
            .await
            .map(|_| ())
            .expect_err("the stale session is not reused");
        assert_eq!(error.code, "connection_failed");
    }

    #[test]
    fn empty_or_relative_directories_fall_back_to_defaults() {
        let config = AppConfig::from_vars(
            Some(OsString::new()),
            Some(OsString::from("relative")),
            Some(OsString::from("known_hosts")),
        );
        assert_eq!(config.dbeaver_home, None);
        assert_eq!(config.data_dir, None);
        assert_eq!(config.ssh_known_hosts, None);
        let config = AppConfig::from_vars(
            None,
            Some(OsString::from("/tmp/pg-pseudo")),
            Some(OsString::from("/tmp/known_hosts")),
        );
        assert_eq!(config.data_dir, Some(PathBuf::from("/tmp/pg-pseudo")));
        assert_eq!(
            config.ssh_known_hosts,
            Some(PathBuf::from("/tmp/known_hosts"))
        );
    }

    fn tunneled(auth: dbeaver::SshAuth) -> ConnectParams {
        ConnectParams {
            host: "db.internal".into(),
            port: 5432,
            database: "app".into(),
            user: "reader".into(),
            password: dbeaver::Secret::new("db-pass"),
            sslmode: dbeaver::SslMode::Require,
            tunnel: Some(dbeaver::TunnelParams {
                host: "bastion.example".into(),
                port: 2222,
                user: "jump".into(),
                auth,
                remote_host: "db.internal".into(),
                remote_port: 6432,
                verify_host_key: false,
            }),
        }
    }

    #[test]
    fn a_dbeaver_ssh_tunnel_maps_to_a_pg_tunnel_with_the_configured_known_hosts() {
        let config = AppConfig {
            ssh_known_hosts: Some(PathBuf::from("/etc/pg-pseudo/known_hosts")),
            ..AppConfig::default()
        };
        let options = connect_options(
            &config,
            &tunneled(dbeaver::SshAuth::PublicKey {
                key_path: PathBuf::from("/keys/id_ed25519"),
                passphrase: Some(dbeaver::Secret::new("phrase")),
            }),
        );
        assert_eq!(
            options.tunnel,
            Some(pg::Tunnel {
                host: "bastion.example".into(),
                port: 2222,
                user: "jump".into(),
                auth: pg::TunnelAuth::PublicKey {
                    key_path: PathBuf::from("/keys/id_ed25519"),
                    passphrase: Some("phrase".into()),
                },
                remote_host: "db.internal".into(),
                remote_port: 6432,
                verify_host_key: false,
                known_hosts: Some(PathBuf::from("/etc/pg-pseudo/known_hosts")),
            })
        );
        let password = connect_options(
            &AppConfig::default(),
            &tunneled(dbeaver::SshAuth::Password(dbeaver::Secret::new("ssh-pass"))),
        );
        let tunnel = password.tunnel.expect("tunnel");
        assert_eq!(tunnel.auth, pg::TunnelAuth::Password("ssh-pass".into()));
        assert_eq!(tunnel.known_hosts, None);
        let agent = connect_options(&AppConfig::default(), &tunneled(dbeaver::SshAuth::Agent));
        assert_eq!(agent.tunnel.expect("tunnel").auth, pg::TunnelAuth::Agent);
    }
}
