use super::connection_profile::SslMode;

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
pub struct ConnectionDefinition {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub sslmode: SslMode,
    pub connection_type: ConnectionType,
    pub auth: AuthMode,
    pub ssh_alias: Option<String>,
}
