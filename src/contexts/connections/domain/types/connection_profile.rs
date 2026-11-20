use std::fmt;
use std::path::PathBuf;

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
    pub fn at_least_require(self) -> Self {
        match self {
            Self::Disable | Self::Allow | Self::Prefer => Self::Require,
            strict => strict,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshAuth {
    PublicKey {
        key_path: PathBuf,
        passphrase: Option<Secret>,
    },
    Password(Secret),
    Agent,
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
