use std::path::Path;

use crate::contexts::connections::domain::types::{
    ConnectOptions, ConnectParams, Hop, HostKeyPolicy, SshAuth, SshHop, Tunnel, TunnelAuth,
};

pub fn connect_options(params: &ConnectParams, known_hosts: Option<&Path>) -> ConnectOptions {
    ConnectOptions {
        host: params.host.clone(),
        port: params.port,
        database: params.database.clone(),
        user: params.user.clone(),
        password: params.password.expose().to_string(),
        sslmode: params.sslmode,
        tunnel: params.tunnel.as_ref().map(|tunnel| Tunnel {
            hops: tunnel
                .hops
                .iter()
                .map(|hop| hop_options(hop, known_hosts))
                .collect(),
            remote_host: tunnel.remote_host.clone(),
            remote_port: tunnel.remote_port,
        }),
    }
}

fn hop_options(hop: &SshHop, configured_known_hosts: Option<&Path>) -> Hop {
    let (verify_host_key, known_hosts, host_key_alias) = match &hop.host_key {
        HostKeyPolicy::Bypass => (false, Vec::new(), None),
        HostKeyPolicy::Verify { known_hosts, alias } => (
            true,
            configured_known_hosts
                .map(|path| vec![path.to_path_buf()])
                .unwrap_or_else(|| known_hosts.clone()),
            alias.clone(),
        ),
    };
    Hop {
        label: hop.label.clone(),
        host: hop.host.clone(),
        port: hop.port,
        user: hop.user.clone(),
        auth: match &hop.auth {
            SshAuth::PublicKey {
                key_path,
                passphrase,
            } => TunnelAuth::PublicKey {
                key_path: key_path.clone(),
                passphrase: passphrase
                    .as_ref()
                    .map(|passphrase| passphrase.expose().to_string()),
            },
            SshAuth::Password(password) => TunnelAuth::Password(password.expose().to_string()),
            SshAuth::Agent => TunnelAuth::Agent,
        },
        verify_host_key,
        known_hosts,
        host_key_alias,
    }
}

#[cfg(test)]
mod unit {
    use std::path::PathBuf;

    use super::*;
    use crate::contexts::connections::domain::types::{
        PasswordSource, Secret, SslMode, TunnelParams,
    };

    fn tunneled(auth: SshAuth, host_key: HostKeyPolicy) -> ConnectParams {
        ConnectParams {
            host: "db.internal".into(),
            port: 5432,
            database: "app".into(),
            user: "reader".into(),
            password: PasswordSource::Resolved(Secret::new("db-pass")),
            sslmode: SslMode::Require,
            tunnel: Some(TunnelParams {
                hops: vec![SshHop {
                    label: "bastion.example".into(),
                    host: "bastion.example".into(),
                    port: 2222,
                    user: "jump".into(),
                    auth,
                    host_key,
                }],
                remote_host: "db.internal".into(),
                remote_port: 6432,
            }),
        }
    }

    fn verified(known_hosts: Vec<PathBuf>) -> HostKeyPolicy {
        HostKeyPolicy::Verify {
            known_hosts,
            alias: Some("bastion-alias".into()),
        }
    }

    #[test]
    fn un_tunnel_ssh_dbeaver_devient_un_tunnel_pg_avec_le_known_hosts_configure() {
        let configured = PathBuf::from("/etc/pg-pseudo/known_hosts");
        let options = connect_options(
            &tunneled(
                SshAuth::PublicKey {
                    key_path: PathBuf::from("/keys/id_ed25519"),
                    passphrase: Some(Secret::new("phrase")),
                },
                verified(vec![PathBuf::from("/home/me/.ssh/known_hosts")]),
            ),
            Some(&configured),
        );
        assert_eq!(
            options.tunnel,
            Some(Tunnel {
                hops: vec![Hop {
                    label: "bastion.example".into(),
                    host: "bastion.example".into(),
                    port: 2222,
                    user: "jump".into(),
                    auth: TunnelAuth::PublicKey {
                        key_path: PathBuf::from("/keys/id_ed25519"),
                        passphrase: Some("phrase".into()),
                    },
                    verify_host_key: true,
                    known_hosts: vec![configured],
                    host_key_alias: Some("bastion-alias".into()),
                }],
                remote_host: "db.internal".into(),
                remote_port: 6432,
            })
        );
    }

    #[test]
    fn sans_known_hosts_configure_les_fichiers_du_saut_sont_conserves() {
        let files = vec![
            PathBuf::from("/home/me/.ssh/known_hosts"),
            PathBuf::from("/etc/ssh/ssh_known_hosts"),
        ];
        let options = connect_options(&tunneled(SshAuth::Agent, verified(files.clone())), None);
        let hop = options.tunnel.expect("tunnel").hops.remove(0);
        assert_eq!(hop.known_hosts, files);
        assert_eq!(hop.auth, TunnelAuth::Agent);
    }

    #[test]
    fn un_saut_sans_verification_ne_porte_aucun_fichier_known_hosts() {
        let configured = PathBuf::from("/etc/pg-pseudo/known_hosts");
        let options = connect_options(
            &tunneled(
                SshAuth::Password(Secret::new("ssh-pass")),
                HostKeyPolicy::Bypass,
            ),
            Some(&configured),
        );
        let hop = options.tunnel.expect("tunnel").hops.remove(0);
        assert!(!hop.verify_host_key);
        assert!(hop.known_hosts.is_empty());
        assert_eq!(hop.auth, TunnelAuth::Password("ssh-pass".into()));
    }
}
