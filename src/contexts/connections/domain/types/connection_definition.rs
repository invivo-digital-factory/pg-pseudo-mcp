use super::connection_profile::SslMode;
use super::read_preference::MongoReadPreference;

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
pub enum MongoTransport {
    Srv { host: String },
    Seed { host: String, port: u16, tls: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MongoSettings {
    pub database: Option<String>,
    pub replica_set: Option<String>,
    pub transport: MongoTransport,
    pub read_preference: Option<MongoReadPreference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineSettings {
    Postgresql {
        host: String,
        port: u16,
        database: String,
        sslmode: SslMode,
    },
    Mongodb(MongoSettings),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionDefinition {
    pub name: String,
    pub user: String,
    pub settings: EngineSettings,
    pub connection_type: ConnectionType,
    pub auth: AuthMode,
    pub ssh_alias: Option<String>,
}
