use super::connection_profile::SslMode;
use crate::contexts::connections::domain::constants::{ENGINE_MONGODB, ENGINE_POSTGRESQL};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    Dev,
    Test,
    Prod,
}

impl ConnectionType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dev => "dev",
            Self::Test => "test",
            Self::Prod => "prod",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "dev" => Some(Self::Dev),
            "test" => Some(Self::Test),
            "prod" => Some(Self::Prod),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMode {
    Pgpass,
    Azure,
    Command(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MongoSettings {
    pub database: Option<String>,
    pub srv: bool,
    pub tls: bool,
    pub replica_set: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineSettings {
    Postgresql { database: String, sslmode: SslMode },
    Mongodb(MongoSettings),
}

impl EngineSettings {
    pub fn engine(&self) -> &'static str {
        match self {
            Self::Postgresql { .. } => ENGINE_POSTGRESQL,
            Self::Mongodb(_) => ENGINE_MONGODB,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionDefinition {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub settings: EngineSettings,
    pub connection_type: ConnectionType,
    pub auth: AuthMode,
    pub ssh_alias: Option<String>,
}
