use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::dbeaver::{self, ConnectParams};
use crate::pg::{self, ConnectOptions, Session};
use crate::pseudo::Pseudonymizer;
use crate::tools::errors::{ErrorCategory, ToolError};

#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    pub dbeaver_home: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            dbeaver_home: std::env::var_os("PG_PSEUDO_DBEAVER_HOME").map(PathBuf::from),
            data_dir: std::env::var_os("PG_PSEUDO_DATA_DIR").map(PathBuf::from),
        }
    }
}

pub struct AppState {
    pub config: AppConfig,
    pseudonymizer: Result<Arc<Pseudonymizer>, String>,
    sessions: Mutex<HashMap<String, Arc<Mutex<Session>>>>,
}

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

    pub async fn session(&self, connection: &str) -> Result<Arc<Mutex<Session>>, ToolError> {
        let params = dbeaver::find_connection(&self.config, connection)?;
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get(connection) {
            return Ok(session.clone());
        }
        let session = pg::connect(&connect_options(&params))
            .await
            .map_err(|error| {
                ToolError::new(
                    ErrorCategory::ConnectionUnavailable,
                    "connection_failed",
                    format!("could not connect to {connection}: {error}"),
                )
                .with_hint("check that the database is reachable from this machine, then retry")
                .retryable()
            })?;
        let session = Arc::new(Mutex::new(session));
        sessions.insert(connection.to_string(), session.clone());
        Ok(session)
    }
}

pub fn connect_options(params: &ConnectParams) -> ConnectOptions {
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
    }
}
