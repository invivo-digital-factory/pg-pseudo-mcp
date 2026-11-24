use serde::Serialize;

use super::connection_profile::{ConnectParams, MongoConnectParams};
use super::connection_source::ConnectionSource;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ConnectionStatus {
    Available,
    Unavailable { reason: String },
    Unsupported { reason: String },
}

#[derive(Debug, Clone)]
pub struct ConnectionEntry {
    pub id: String,
    pub name: String,
    pub declared_name: String,
    pub source: ConnectionSource,
    pub folder: Option<String>,
    pub project: String,
    pub engine: String,
    pub connection_type: String,
    pub database: Option<String>,
    pub status: ConnectionStatus,
    pub params: Option<ConnectParams>,
    pub mongo: Option<MongoConnectParams>,
}

#[derive(Debug, Clone)]
pub struct Discovery {
    pub connections: Vec<ConnectionEntry>,
    pub hint: Option<String>,
}
