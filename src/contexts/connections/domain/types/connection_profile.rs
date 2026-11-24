use std::fmt;
use std::path::PathBuf;

use super::connection_definition::MongoTransport;
use super::password_source::PasswordSource;
use super::secret::Secret;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SslMode {
    Disable,
    Allow,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

impl SslMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disable => "disable",
            Self::Allow => "allow",
            Self::Prefer => "prefer",
            Self::Require => "require",
            Self::VerifyCa => "verify-ca",
            Self::VerifyFull => "verify-full",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "disable" => Some(Self::Disable),
            "allow" => Some(Self::Allow),
            "prefer" => Some(Self::Prefer),
            "require" => Some(Self::Require),
            "verify-ca" => Some(Self::VerifyCa),
            "verify-full" => Some(Self::VerifyFull),
            _ => None,
        }
    }

    pub fn at_least_require(self) -> Self {
        match self {
            Self::Disable | Self::Allow | Self::Prefer => Self::Require,
            strict => strict,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshAgent {
    Environment,
    Socket(PathBuf),
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSshKeys {
    pub identity_files: Vec<PathBuf>,
    pub agent: SshAgent,
    pub identities_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshAuth {
    PublicKey {
        key_path: PathBuf,
        passphrase: Option<Secret>,
    },
    Password(Secret),
    Agent,
    OpenSsh(OpenSshKeys),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyPolicy {
    Bypass,
    Verify {
        known_hosts: Vec<PathBuf>,
        alias: Option<String>,
    },
}

impl HostKeyPolicy {
    pub fn is_verified(&self) -> bool {
        matches!(self, Self::Verify { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshHop {
    pub label: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: SshAuth,
    pub host_key: HostKeyPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelParams {
    pub hops: Vec<SshHop>,
    pub remote_host: String,
    pub remote_port: u16,
}

#[derive(Clone)]
pub struct ConnectParams {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: PasswordSource,
    pub sslmode: SslMode,
    pub tunnel: Option<TunnelParams>,
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
            .field("tunnel", &self.tunnel)
            .finish()
    }
}

#[derive(Clone)]
pub struct MongoConnectParams {
    pub transport: MongoTransport,
    pub replica_set: Option<String>,
    pub database: Option<String>,
    pub user: String,
    pub password_command: Vec<String>,
    pub ssh_hops: Option<Vec<SshHop>>,
}

impl fmt::Debug for MongoConnectParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MongoConnectParams")
            .field("transport", &self.transport)
            .field("replica_set", &self.replica_set)
            .field("database", &self.database)
            .field("user", &self.user)
            .field("password_command", &"<command>")
            .field("ssh_hops", &self.ssh_hops)
            .finish()
    }
}
