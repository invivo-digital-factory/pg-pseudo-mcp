use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use crate::contexts::connections::domain::constants::CONNECT_DEADLINE;
use crate::contexts::connections::domain::errors::ConnectionError;
use crate::contexts::connections::domain::repositories::{ConnectionCatalog, SessionPool};
use crate::contexts::connections::domain::services::{connect_options, resolve_connection};
use crate::contexts::connections::domain::types::{ExecutionLimits, QueryOutput};
use crate::contexts::connections::infrastructure::data_sources::postgres::{
    Session, connect_within,
};

type SessionSlot = Arc<Mutex<Option<Session>>>;

pub struct PostgresSessionPool {
    catalog: Arc<dyn ConnectionCatalog>,
    known_hosts: Option<PathBuf>,
    sessions: Mutex<HashMap<String, SessionSlot>>,
    connect_deadline: Duration,
}

impl PostgresSessionPool {
    pub fn new(catalog: Arc<dyn ConnectionCatalog>, known_hosts: Option<PathBuf>) -> Self {
        Self {
            catalog,
            known_hosts,
            sessions: Mutex::new(HashMap::new()),
            connect_deadline: CONNECT_DEADLINE,
        }
    }

    pub fn with_connect_deadline(mut self, connect_deadline: Duration) -> Self {
        self.connect_deadline = connect_deadline;
        self
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
            .expect("idle pool")
            .entry(connection.to_string())
            .or_default()
            .clone();
        *slot.try_lock().expect("idle slot") = Some(session);
    }
}

impl SessionPool for PostgresSessionPool {
    async fn run_read_only(
        &self,
        connection: &str,
        sql: &str,
        limits: ExecutionLimits,
    ) -> Result<QueryOutput, ConnectionError> {
        let params = resolve_connection(&self.catalog.discover(), connection)?;
        let options = connect_options(&params, self.known_hosts.as_deref());
        let slot = self.slot(connection).await;
        let mut cached = slot.lock_owned().await;
        let session = match cached.take() {
            Some(session) if session.options() == &options => session,
            _ => connect_within(&options, self.connect_deadline)
                .await
                .map_err(|source| ConnectionError::ConnectFailed {
                    connection: connection.to_string(),
                    source,
                })?,
        };
        let session = cached.insert(session);
        session
            .run_read_only(sql, limits.max_rows, limits.timeout_ms)
            .await
            .map_err(ConnectionError::Session)
    }
}

#[cfg(test)]
mod integration {
    use std::path::Path;
    use std::time::Instant;

    use serde_json::{Map, Value, json};
    use tokio::net::TcpListener;

    use super::*;
    use crate::contexts::connections::infrastructure::data_sources::dbeaver::credentials::encrypt_for_tests;
    use crate::contexts::connections::infrastructure::repositories::DbeaverConnectionCatalog;

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

    fn pool_for(root: &Path) -> PostgresSessionPool {
        PostgresSessionPool::new(
            Arc::new(DbeaverConnectionCatalog::new(Some(root.to_path_buf()))),
            None,
        )
    }

    fn limits() -> ExecutionLimits {
        ExecutionLimits {
            max_rows: 10,
            timeout_ms: 1_000,
        }
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
    async fn un_serveur_qui_cale_apres_l_ouverture_ne_bloque_pas_les_autres_connexions() {
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
        let pool =
            Arc::new(pool_for(root.path()).with_connect_deadline(Duration::from_millis(500)));
        let started = Instant::now();
        let stalled = tokio::spawn({
            let pool = pool.clone();
            async move { pool.run_read_only("stalled", "SELECT 1", limits()).await }
        });
        tokio::time::sleep(Duration::from_millis(100)).await;
        let other = tokio::time::timeout(
            Duration::from_secs(1),
            pool.run_read_only("refused", "SELECT 1", limits()),
        )
        .await
        .expect("another connection is not blocked by the stalled one");
        assert!(other.is_err());
        let stalled = tokio::time::timeout(Duration::from_secs(3), stalled)
            .await
            .expect("the stalled connection gives up within its bound")
            .expect("task");
        let error = stalled.expect_err("stalled connection fails");
        assert!(error.to_string().contains("timed out"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[tokio::test]
    async fn un_mot_de_passe_modifie_dans_dbeaver_remplace_la_session_en_cache() {
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
        let pool = pool_for(root.path());
        let params = resolve_connection(&pool.catalog.discover(), "app").expect("params");
        pool.cache_for_tests("app", Session::idle(connect_options(&params, None)));
        let reused = pool
            .run_read_only("app", "SELECT 1", limits())
            .await
            .expect_err("the idle cached session reconnects to the closed port");
        assert!(matches!(reused, ConnectionError::Session(_)), "{reused:?}");
        write_workspace(
            root.path(),
            &[Endpoint {
                name: "app",
                port,
                password: "new",
            }],
        );
        let error = pool
            .run_read_only("app", "SELECT 1", limits())
            .await
            .expect_err("the stale session is not reused");
        assert!(
            matches!(error, ConnectionError::ConnectFailed { .. }),
            "{error:?}"
        );
    }
}
