use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use crate::contexts::connections::domain::constants::CONNECT_DEADLINE;
use crate::contexts::connections::domain::errors::{ConnectionError, SessionError};
use crate::contexts::connections::domain::repositories::{
    AzureSignInTokenProvider, ConnectionCatalog, SessionPool,
};
use crate::contexts::connections::domain::services::{connect_options, resolve_connection};
use crate::contexts::connections::domain::types::{ConnectOptions, ExecutionLimits, QueryOutput};
use crate::contexts::connections::infrastructure::data_sources::postgres::{
    Session, connect_within,
};
use crate::contexts::connections::infrastructure::mappers::redact_connection_identity;
use crate::contexts::connections::infrastructure::repositories::session_cache::{
    SessionCache, Slot,
};

const MAX_OTHER_DATABASE_SESSIONS: usize = 4;

pub struct PostgresSessionPool {
    catalog: Arc<dyn ConnectionCatalog>,
    known_hosts: Option<PathBuf>,
    tokens: Arc<dyn AzureSignInTokenProvider>,
    sessions: Mutex<SessionCache<Session>>,
    connect_deadline: Duration,
}

impl PostgresSessionPool {
    pub fn new(
        catalog: Arc<dyn ConnectionCatalog>,
        known_hosts: Option<PathBuf>,
        tokens: Arc<dyn AzureSignInTokenProvider>,
    ) -> Self {
        Self {
            catalog,
            known_hosts,
            tokens,
            sessions: Mutex::new(SessionCache::new(MAX_OTHER_DATABASE_SESSIONS)),
            connect_deadline: CONNECT_DEADLINE,
        }
    }

    pub fn with_connect_deadline(mut self, connect_deadline: Duration) -> Self {
        self.connect_deadline = connect_deadline;
        self
    }

    async fn slot(
        &self,
        connection: &str,
        database: &str,
        default_database: bool,
    ) -> Slot<Session> {
        self.sessions
            .lock()
            .await
            .slot(connection, database, default_database)
    }

    async fn forget(&self, connection: &str, database: &str, slot: &Slot<Session>) {
        self.sessions
            .lock()
            .await
            .forget(connection, database, slot);
    }

    async fn connect(
        &self,
        connection: &str,
        options: ConnectOptions,
        renewable_password: bool,
    ) -> Result<Session, ConnectionError> {
        let connect_failed = |source| ConnectionError::ConnectFailed {
            connection: connection.to_string(),
            source: redact_connection_identity(source, &options),
        };
        match connect_within(&options, self.connect_deadline).await {
            Err(refused) if renewable_password && refused.is_authentication_failure() => self
                .retry_with_renewed_token(&options)
                .await?
                .map_err(connect_failed),
            other => other.map_err(connect_failed),
        }
    }

    async fn retry_with_renewed_token(
        &self,
        options: &ConnectOptions,
    ) -> Result<Result<Session, SessionError>, ConnectionError> {
        let tokens = self.tokens.clone();
        let token = tokio::task::spawn_blocking(move || tokens.renewed_access_token())
            .await
            .map_err(|_| ConnectionError::discovery_interrupted())?
            .map_err(|error| ConnectionError::Unavailable {
                reason: error.to_string(),
            })?;
        let options = ConnectOptions {
            password: token.expose().to_string(),
            ..options.clone()
        };
        Ok(connect_within(&options, self.connect_deadline).await)
    }

    #[cfg(test)]
    fn cached_sessions_for_tests(&self) -> usize {
        self.sessions.try_lock().expect("idle pool").len()
    }

    #[cfg(test)]
    fn cache_for_tests(&self, connection: &str, database: &str, session: Session) {
        let slot = self
            .sessions
            .try_lock()
            .expect("idle pool")
            .slot(connection, database, true);
        *slot.try_lock().expect("idle slot") = Some(session);
    }
}

impl SessionPool for PostgresSessionPool {
    async fn run_read_only(
        &self,
        connection: &str,
        database: Option<&str>,
        sql: &str,
        limits: ExecutionLimits,
    ) -> Result<QueryOutput, ConnectionError> {
        let catalog = self.catalog.clone();
        let database_unrequested = database.is_none_or(|name| name.trim().is_empty());
        let requested = database
            .filter(|name| !name.trim().is_empty())
            .map(str::to_string);
        let discovery =
            tokio::task::spawn_blocking(move || catalog.discover_in(requested.as_deref()))
                .await
                .map_err(|_| ConnectionError::discovery_interrupted())?;
        let params = resolve_connection(&discovery, connection)?;
        let options = connect_options(&params, self.known_hosts.as_deref());
        let slot = self
            .slot(connection, &params.database, database_unrequested)
            .await;
        let mut cached = slot.clone().lock_owned().await;
        let session = match cached.take() {
            Some(session) if session.options() == &options => session,
            _ => {
                match self
                    .connect(connection, options, params.password_is_azure_token)
                    .await
                {
                    Ok(session) => session,
                    Err(error) => {
                        drop(cached);
                        self.forget(connection, &params.database, &slot).await;
                        return Err(error);
                    }
                }
            }
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
    use std::str::FromStr;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    use serde_json::{Map, Value, json};
    use tokio::net::TcpListener;

    use super::*;
    use crate::contexts::connections::domain::errors::AccessTokenError;
    use crate::contexts::connections::domain::types::{
        Cell, ConnectParams, ConnectionEntry, ConnectionStatus, Discovery, Secret, SslMode,
    };
    use crate::contexts::connections::infrastructure::data_sources::dbeaver::credentials::encrypt_for_tests;
    use crate::contexts::connections::infrastructure::repositories::DbeaverConnectionCatalog;
    use crate::contexts::connections::test::database::database_url;

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

    struct NoTokens;

    impl AzureSignInTokenProvider for NoTokens {
        fn access_token(&self) -> Result<Secret, AccessTokenError> {
            Err(AccessTokenError::CliNotInstalled)
        }

        fn renewed_access_token(&self) -> Result<Secret, AccessTokenError> {
            Err(AccessTokenError::CliNotInstalled)
        }
    }

    fn pool_for(root: &Path) -> PostgresSessionPool {
        PostgresSessionPool::new(
            Arc::new(DbeaverConnectionCatalog::new(
                Some(root.to_path_buf()),
                Arc::new(NoTokens),
            )),
            None,
            Arc::new(NoTokens),
        )
    }

    struct FixedCatalog(ConnectParams);

    impl ConnectionCatalog for FixedCatalog {
        fn discover_in(&self, database: Option<&str>) -> Discovery {
            let mut params = self.0.clone();
            if let Some(database) = database {
                params.database = database.to_string();
            }
            Discovery {
                connections: vec![ConnectionEntry {
                    id: "azure".into(),
                    name: "azure".into(),
                    folder: None,
                    project: "General".into(),
                    engine: "postgresql".into(),
                    connection_type: "dev".into(),
                    database: Some(params.database.clone()),
                    status: ConnectionStatus::Available,
                    params: Some(params),
                }],
                hint: None,
            }
        }
    }

    struct ScriptedTokens {
        renewed: Result<Secret, AccessTokenError>,
        renewals: AtomicUsize,
    }

    impl ScriptedTokens {
        fn new(renewed: Result<Secret, AccessTokenError>) -> Arc<Self> {
            Arc::new(Self {
                renewed,
                renewals: AtomicUsize::new(0),
            })
        }

        fn renewals(&self) -> usize {
            self.renewals.load(Ordering::SeqCst)
        }
    }

    impl AzureSignInTokenProvider for ScriptedTokens {
        fn access_token(&self) -> Result<Secret, AccessTokenError> {
            Err(AccessTokenError::CliNotInstalled)
        }

        fn renewed_access_token(&self) -> Result<Secret, AccessTokenError> {
            self.renewals.fetch_add(1, Ordering::SeqCst);
            self.renewed.clone()
        }
    }

    fn local_params(password: &str, azure: bool) -> ConnectParams {
        let config = tokio_postgres::Config::from_str(&database_url()).expect("database url");
        let host = match config.get_hosts().first() {
            Some(tokio_postgres::config::Host::Tcp(host)) => host.clone(),
            _ => "localhost".to_string(),
        };
        ConnectParams {
            host,
            port: config.get_ports().first().copied().unwrap_or(5432),
            database: config.get_dbname().unwrap_or("pgpseudo").to_string(),
            user: config.get_user().unwrap_or("pgpseudo").to_string(),
            password: Secret::new(password),
            password_is_azure_token: azure,
            sslmode: SslMode::Disable,
            tunnel: None,
        }
    }

    fn real_password() -> Secret {
        let config = tokio_postgres::Config::from_str(&database_url()).expect("database url");
        Secret::new(String::from_utf8_lossy(config.get_password().expect("password")).into_owned())
    }

    fn azure_pool(params: ConnectParams, tokens: Arc<ScriptedTokens>) -> PostgresSessionPool {
        PostgresSessionPool::new(Arc::new(FixedCatalog(params)), None, tokens)
    }

    #[tokio::test]
    #[ignore]
    async fn un_jeton_azure_refuse_est_remplace_par_un_jeton_neuf_exactement_une_fois() {
        let tokens = ScriptedTokens::new(Ok(real_password()));
        let pool = azure_pool(local_params("expired-token", true), tokens.clone());
        let output = pool
            .run_read_only("azure", None, "SELECT 1", limits())
            .await
            .expect("the retry with a fresh token connects");
        assert_eq!(output.rows.len(), 1);
        assert_eq!(tokens.renewals(), 1);
    }

    #[tokio::test]
    #[ignore]
    async fn un_jeton_neuf_encore_refuse_echoue_apres_une_seule_relance() {
        let tokens = ScriptedTokens::new(Ok(Secret::new("still-wrong")));
        let pool = azure_pool(local_params("expired-token", true), tokens.clone());
        let error = pool
            .run_read_only("azure", None, "SELECT 1", limits())
            .await
            .expect_err("both tokens are refused");
        assert!(
            matches!(
                &error,
                ConnectionError::ConnectFailed { source, .. } if source.is_authentication_failure()
            ),
            "{error:?}"
        );
        assert_eq!(tokens.renewals(), 1);
        assert!(!error.to_string().contains("still-wrong"));
    }

    #[tokio::test]
    #[ignore]
    async fn un_renouvellement_impossible_rend_l_indication_de_connexion_az() {
        let tokens = ScriptedTokens::new(Err(AccessTokenError::SignInRequired));
        let pool = azure_pool(local_params("expired-token", true), tokens.clone());
        let error = pool
            .run_read_only("azure", None, "SELECT 1", limits())
            .await
            .expect_err("no fresh token");
        assert!(
            matches!(
                &error,
                ConnectionError::Unavailable { reason } if reason.contains("az login")
            ),
            "{error:?}"
        );
        assert_eq!(tokens.renewals(), 1);
    }

    #[tokio::test]
    #[ignore]
    async fn un_mot_de_passe_refuse_hors_azure_n_est_jamais_retente() {
        let tokens = ScriptedTokens::new(Ok(real_password()));
        let pool = azure_pool(local_params("wrong", false), tokens.clone());
        let error = pool
            .run_read_only("azure", None, "SELECT 1", limits())
            .await
            .expect_err("a stored password is not renewed");
        assert!(matches!(error, ConnectionError::ConnectFailed { .. }));
        assert_eq!(tokens.renewals(), 0);
    }

    #[tokio::test]
    #[ignore]
    async fn un_mot_de_passe_refuse_rend_le_code_et_le_message_du_serveur() {
        let tokens = ScriptedTokens::new(Ok(real_password()));
        let pool = azure_pool(local_params("hunter2-wrong", false), tokens);
        let error = pool
            .run_read_only("azure", None, "SELECT 1", limits())
            .await
            .expect_err("the password is refused");
        let text = error.to_string();
        assert!(text.contains("28P01"), "{text}");
        assert!(text.contains("password authentication failed"), "{text}");
        assert!(!text.contains("hunter2-wrong"), "{text}");
    }

    #[tokio::test]
    #[ignore]
    async fn une_base_absente_rend_le_code_et_le_message_du_serveur() {
        let tokens = ScriptedTokens::new(Ok(real_password()));
        let mut params = local_params("unused", false);
        params.password = real_password();
        params.database = "absent_database_for_test".to_string();
        let pool = azure_pool(params, tokens);
        let error = pool
            .run_read_only("azure", None, "SELECT 1", limits())
            .await
            .expect_err("the database does not exist");
        let text = error.to_string();
        assert!(text.contains("3D000"), "{text}");
        assert!(text.contains("absent_database_for_test"), "{text}");
        assert!(text.contains("does not exist"), "{text}");
    }

    #[tokio::test]
    async fn un_port_ferme_rend_la_cause_reseau_et_pas_seulement_un_libelle_generique() {
        let root = tempfile::tempdir().expect("dbeaver dir");
        write_workspace(
            root.path(),
            &[Endpoint {
                name: "closed",
                port: refused_port().await,
                password: "p",
            }],
        );
        let pool = pool_for(root.path());
        let error = pool
            .run_read_only("closed", None, "SELECT 1", limits())
            .await
            .expect_err("nothing listens");
        let text = error.to_string().to_lowercase();
        assert!(text.contains("refused"), "{text}");
    }

    #[tokio::test]
    async fn un_premier_echec_de_connexion_ne_garde_aucune_place_en_cache() {
        let root = tempfile::tempdir().expect("dbeaver dir");
        write_workspace(
            root.path(),
            &[Endpoint {
                name: "closed",
                port: refused_port().await,
                password: "p",
            }],
        );
        let pool = pool_for(root.path());
        for database in [None, Some("other"), Some("another")] {
            pool.run_read_only("closed", database, "SELECT 1", limits())
                .await
                .expect_err("nothing listens");
        }
        assert_eq!(pool.cached_sessions_for_tests(), 0);
    }

    async fn backend_and_database(
        pool: &PostgresSessionPool,
        database: Option<&str>,
    ) -> (Cell, Cell) {
        let output = pool
            .run_read_only(
                "azure",
                database,
                "SELECT pg_backend_pid(), current_database()::text",
                limits(),
            )
            .await
            .expect("the database is reachable");
        let row = &output.rows[0];
        (row[0].clone(), row[1].clone())
    }

    #[tokio::test]
    #[ignore]
    async fn deux_bases_d_une_meme_connexion_ont_chacune_leur_session_reutilisee() {
        let tokens = ScriptedTokens::new(Ok(real_password()));
        let pool = azure_pool(local_params(real_password().expose(), false), tokens);
        let (default_pid, default_name) = backend_and_database(&pool, None).await;
        let (other_pid, other_name) = backend_and_database(&pool, Some("postgres")).await;
        assert_eq!(other_name, Cell::Text("postgres".into()));
        assert_ne!(default_name, other_name);
        assert_ne!(default_pid, other_pid);
        assert_eq!(backend_and_database(&pool, None).await.0, default_pid);
        assert_eq!(
            backend_and_database(&pool, Some("postgres")).await.0,
            other_pid
        );
    }

    #[tokio::test]
    #[ignore]
    async fn la_base_par_defaut_demandee_explicitement_reutilise_la_session_existante() {
        let tokens = ScriptedTokens::new(Ok(real_password()));
        let params = local_params(real_password().expose(), false);
        let default = params.database.clone();
        let pool = azure_pool(params, tokens);
        let (pid, _) = backend_and_database(&pool, None).await;
        assert_eq!(backend_and_database(&pool, Some(&default)).await.0, pid);
    }

    #[tokio::test]
    #[ignore]
    async fn une_base_inconnue_rend_l_erreur_lisible_du_serveur() {
        let tokens = ScriptedTokens::new(Ok(real_password()));
        let pool = azure_pool(local_params(real_password().expose(), false), tokens);
        let error = pool
            .run_read_only(
                "azure",
                Some("absent_database_for_test"),
                "SELECT 1",
                limits(),
            )
            .await
            .expect_err("the database does not exist");
        let text = error.to_string();
        assert!(text.contains("3D000"), "{text}");
        assert!(text.contains("absent_database_for_test"), "{text}");
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
            async move {
                pool.run_read_only("stalled", None, "SELECT 1", limits())
                    .await
            }
        });
        tokio::time::sleep(Duration::from_millis(100)).await;
        let other = tokio::time::timeout(
            Duration::from_secs(1),
            pool.run_read_only("refused", None, "SELECT 1", limits()),
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
        pool.cache_for_tests(
            "app",
            &params.database,
            Session::idle(connect_options(&params, None)),
        );
        let reused = pool
            .run_read_only("app", None, "SELECT 1", limits())
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
            .run_read_only("app", None, "SELECT 1", limits())
            .await
            .expect_err("the stale session is not reused");
        assert!(
            matches!(error, ConnectionError::ConnectFailed { .. }),
            "{error:?}"
        );
    }
}
