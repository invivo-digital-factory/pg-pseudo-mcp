use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SslMode {
    Disable,
    Allow,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ConnectOptions {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: String,
    pub sslmode: SslMode,
    pub tunnel: Option<Tunnel>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Tunnel {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: TunnelAuth,
    pub remote_host: String,
    pub remote_port: u16,
    pub verify_host_key: bool,
    pub known_hosts: Option<PathBuf>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum TunnelAuth {
    PublicKey {
        key_path: PathBuf,
        passphrase: Option<String>,
    },
    Password(String),
    Agent,
}

impl fmt::Debug for ConnectOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectOptions")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("database", &self.database)
            .field("user", &self.user)
            .field("password", &"<redacted>")
            .field("sslmode", &self.sslmode)
            .field("tunnel", &self.tunnel)
            .finish()
    }
}

impl fmt::Debug for Tunnel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tunnel")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("user", &self.user)
            .field("auth", &self.auth)
            .field("remote_host", &self.remote_host)
            .field("remote_port", &self.remote_port)
            .field("verify_host_key", &self.verify_host_key)
            .field("known_hosts", &self.known_hosts)
            .finish()
    }
}

impl fmt::Debug for TunnelAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PublicKey {
                key_path,
                passphrase,
            } => f
                .debug_struct("PublicKey")
                .field("key_path", key_path)
                .field("passphrase", &passphrase.as_ref().map(|_| "<redacted>"))
                .finish(),
            Self::Password(_) => f.debug_tuple("Password").field(&"<redacted>").finish(),
            Self::Agent => f.write_str("Agent"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_never_contains_the_password() {
        let options = ConnectOptions {
            host: "localhost".into(),
            port: 5432,
            database: "app".into(),
            user: "reader".into(),
            password: "s3cr3t-value".into(),
            sslmode: SslMode::Prefer,
            tunnel: None,
        };
        let printed = format!("{options:?}");
        assert!(!printed.contains("s3cr3t-value"));
        assert!(printed.contains("reader"));
    }

    fn tunnel(auth: TunnelAuth) -> Tunnel {
        Tunnel {
            host: "bastion.example".into(),
            port: 2222,
            user: "jump".into(),
            auth,
            remote_host: "db.internal".into(),
            remote_port: 5432,
            verify_host_key: true,
            known_hosts: None,
        }
    }

    #[test]
    fn debug_output_never_contains_tunnel_secrets() {
        let options = ConnectOptions {
            host: "db.internal".into(),
            port: 5432,
            database: "app".into(),
            user: "reader".into(),
            password: "db-pass".into(),
            sslmode: SslMode::Require,
            tunnel: Some(tunnel(TunnelAuth::PublicKey {
                key_path: "/keys/id_ed25519".into(),
                passphrase: Some("key-phrase-81".into()),
            })),
        };
        let printed = format!("{options:?}");
        assert!(!printed.contains("key-phrase-81"), "{printed}");
        assert!(printed.contains("bastion.example"), "{printed}");
        assert!(printed.contains("/keys/id_ed25519"), "{printed}");
        let printed = format!("{:?}", tunnel(TunnelAuth::Password("ssh-pass-19".into())));
        assert!(!printed.contains("ssh-pass-19"), "{printed}");
        assert!(printed.contains("Password"), "{printed}");
    }
}
