pub mod credentials;
pub mod variables;
mod workspace;

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

pub use credentials::Secret;

use crate::state::AppConfig;
use crate::tools::errors::{ErrorCategory, ToolError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ConnectionStatus {
    Available,
    Unavailable { reason: String },
    Unsupported { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SslMode {
    Disable,
    Allow,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

#[derive(Clone)]
pub struct ConnectParams {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: Secret,
    pub sslmode: SslMode,
}

impl fmt::Debug for ConnectParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectParams")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("database", &self.database)
            .field("user", &self.user)
            .field("password", &self.password)
            .field("sslmode", &self.sslmode)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionEntry {
    pub id: String,
    pub name: String,
    pub folder: Option<String>,
    pub project: String,
    pub engine: String,
    pub connection_type: String,
    pub database: Option<String>,
    pub status: ConnectionStatus,
    pub params: Option<ConnectParams>,
}

#[derive(Debug, Clone)]
pub struct Discovery {
    pub connections: Vec<ConnectionEntry>,
    pub hint: Option<String>,
}

pub fn default_dbeaver_home() -> Option<PathBuf> {
    let dirs = directories::BaseDirs::new()?;
    if cfg!(target_os = "macos") {
        Some(dirs.home_dir().join("Library").join("DBeaverData"))
    } else {
        Some(dirs.data_dir().join("DBeaverData"))
    }
}

pub fn discover(dbeaver_home: Option<&Path>) -> Discovery {
    let Some(root) = dbeaver_home
        .map(Path::to_path_buf)
        .or_else(default_dbeaver_home)
    else {
        return Discovery {
            connections: Vec::new(),
            hint: Some(
                "DBeaver data directory could not be located; set PG_PSEUDO_DBEAVER_HOME"
                    .to_string(),
            ),
        };
    };
    if !root.join("workspace6").is_dir() {
        return Discovery {
            connections: Vec::new(),
            hint: Some(format!(
                "no DBeaver workspace found at {}; set PG_PSEUDO_DBEAVER_HOME to the DBeaverData directory",
                root.display()
            )),
        };
    }
    let mut connections = workspace::load(&root);
    disambiguate(&mut connections);
    connections.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    let hint = connections
        .is_empty()
        .then(|| "the DBeaver workspace contains no connections".to_string());
    Discovery { connections, hint }
}

fn disambiguate(connections: &mut [ConnectionEntry]) {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for entry in connections.iter() {
        *counts.entry(entry.name.clone()).or_default() += 1;
    }
    for entry in connections.iter_mut() {
        if counts.get(&entry.name).copied().unwrap_or(0) > 1 {
            let digest = Sha256::digest(format!("{}/{}", entry.project, entry.id).as_bytes());
            let suffix = hex::encode(&digest[..2]);
            entry.name = format!("{} ({suffix})", entry.name);
        }
    }
}

pub fn find_connection(config: &AppConfig, name: &str) -> Result<ConnectParams, ToolError> {
    let discovery = discover(config.dbeaver_home.as_deref());
    let Some(entry) = discovery
        .connections
        .iter()
        .find(|entry| entry.name == name)
    else {
        let available: Vec<&str> = discovery
            .connections
            .iter()
            .filter(|entry| entry.status == ConnectionStatus::Available)
            .map(|entry| entry.name.as_str())
            .collect();
        let hint = if available.is_empty() {
            "no connection is available; call list_connections".to_string()
        } else {
            format!("available connections: {}", available.join(", "))
        };
        return Err(ToolError::new(
            ErrorCategory::UnknownConnection,
            "unknown_connection",
            format!("unknown connection '{name}'"),
        )
        .with_hint(hint));
    };
    match (&entry.status, &entry.params) {
        (ConnectionStatus::Available, Some(params)) => Ok(params.clone()),
        (ConnectionStatus::Unsupported { reason }, _) => Err(ToolError::new(
            ErrorCategory::ConnectionUnavailable,
            "connection_unavailable",
            reason.clone(),
        )
        .with_hint("this connection cannot be used by this server; pick another one")),
        (ConnectionStatus::Unavailable { reason }, _) => Err(ToolError::new(
            ErrorCategory::ConnectionUnavailable,
            "connection_unavailable",
            reason.clone(),
        )
        .with_hint("ask the developer to fix the connection in DBeaver")),
        (ConnectionStatus::Available, None) => Err(ToolError::new(
            ErrorCategory::Internal,
            "missing_connect_params",
            "connection has no resolved parameters",
        )),
    }
}
