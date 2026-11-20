use std::path::PathBuf;

use super::connection_definition::AuthMode;
use super::connection_profile::SslMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbeaverTunnel {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub key_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbeaverDefinition {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub sslmode: SslMode,
    pub connection_type: String,
    pub pgpass_auth: bool,
    pub tunnel: Option<DbeaverTunnel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedConnection {
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbeaverRead {
    pub definitions: Vec<DbeaverDefinition>,
    pub skipped: Vec<SkippedConnection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostBlock {
    pub alias: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub identity_file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedConnection {
    pub name: String,
    pub auth: AuthMode,
    pub ssh_alias: Option<String>,
    pub alias_proposed: bool,
    pub type_defaulted_from: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReport {
    pub path: PathBuf,
    pub imported: Vec<ImportedConnection>,
    pub skipped: Vec<SkippedConnection>,
    pub host_blocks: Vec<HostBlock>,
}
