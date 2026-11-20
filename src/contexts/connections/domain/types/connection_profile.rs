use std::fmt;
use std::path::PathBuf;

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
pub struct TunnelParams {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: SshAuth,
    pub remote_host: String,
    pub remote_port: u16,
    pub verify_host_key: bool,
}

#[derive(Clone)]
pub struct ConnectParams {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: Secret,
    pub password_is_azure_token: bool,
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
            .field("password_is_azure_token", &self.password_is_azure_token)
            .field("sslmode", &self.sslmode)
            .field("tunnel", &self.tunnel)
            .finish()
    }
}
