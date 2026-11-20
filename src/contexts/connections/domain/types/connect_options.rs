use std::fmt;
use std::path::PathBuf;

use super::connection_profile::{OpenSshKeys, SslMode};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tunnel {
    pub hops: Vec<Hop>,
    pub remote_host: String,
    pub remote_port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hop {
    pub label: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: TunnelAuth,
    pub verify_host_key: bool,
    pub known_hosts: Vec<PathBuf>,
    pub host_key_alias: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum TunnelAuth {
    PublicKey {
        key_path: PathBuf,
        passphrase: Option<String>,
    },
    Password(String),
    Agent,
    OpenSsh(OpenSshKeys),
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
            Self::OpenSsh(keys) => f.debug_tuple("OpenSsh").field(keys).finish(),
        }
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn l_affichage_debug_des_options_ne_contient_jamais_le_mot_de_passe() {
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

    fn hop(host: &str, user: &str, auth: TunnelAuth) -> Hop {
        Hop {
            label: host.into(),
            host: host.into(),
            port: 2222,
            user: user.into(),
            auth,
            verify_host_key: true,
            known_hosts: Vec::new(),
            host_key_alias: None,
        }
    }

    fn tunnel(auth: TunnelAuth) -> Tunnel {
        Tunnel {
            hops: vec![hop("bastion.example", "jump", auth)],
            remote_host: "db.internal".into(),
            remote_port: 5432,
        }
    }

    #[test]
    fn l_affichage_debug_des_options_ne_contient_jamais_les_secrets_du_tunnel() {
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

    #[test]
    fn l_affichage_debug_d_un_tunnel_a_deux_sauts_ne_contient_aucun_secret() {
        let two_hops = Tunnel {
            hops: vec![
                hop(
                    "jump.example",
                    "first",
                    TunnelAuth::Password("first-pass-61".into()),
                ),
                hop(
                    "bastion.example",
                    "second",
                    TunnelAuth::PublicKey {
                        key_path: "/keys/id_ed25519".into(),
                        passphrase: Some("second-phrase-77".into()),
                    },
                ),
            ],
            remote_host: "db.internal".into(),
            remote_port: 5432,
        };
        let printed = format!("{two_hops:?}");
        assert!(!printed.contains("first-pass-61"), "{printed}");
        assert!(!printed.contains("second-phrase-77"), "{printed}");
        assert!(printed.contains("jump.example"), "{printed}");
        assert!(printed.contains("bastion.example"), "{printed}");
    }
}
