use std::path::Path;

use crate::contexts::connections::domain::constants::MONGODB_DEFAULT_PORT;
use crate::contexts::connections::domain::types::{
    ConnectOptions, ConnectParams, Hop, HostKeyPolicy, MongoConnectParams, MongoTransport, SshAuth,
    SshHop, Tunnel, TunnelAuth,
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

pub fn mongo_tunnel(params: &MongoConnectParams, known_hosts: Option<&Path>) -> Option<Tunnel> {
    let (remote_host, remote_port) = match &params.transport {
        MongoTransport::Srv { host } => (host.clone(), MONGODB_DEFAULT_PORT),
        MongoTransport::Seed { host, port, .. } => (host.clone(), *port),
    };
    params.ssh_hops.as_ref().map(|hops| Tunnel {
        hops: hops
            .iter()
            .map(|hop| hop_options(hop, known_hosts))
            .collect(),
        remote_host,
        remote_port,
    })
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
            SshAuth::OpenSsh(keys) => TunnelAuth::OpenSsh(keys.clone()),
        },
        verify_host_key,
        known_hosts,
        host_key_alias,
    }
}

#[cfg(test)]
mod unit {
    use crate::contexts::connections::domain::types::MongoReadPreference;
    use std::path::PathBuf;

    use super::*;
    use crate::contexts::connections::domain::types::{
        PasswordSource, Secret, SslMode, TunnelParams,
    };

    fn mongo(transport: MongoTransport, ssh_hops: Option<Vec<SshHop>>) -> MongoConnectParams {
        MongoConnectParams {
            read_preference: MongoReadPreference::default(),
            transport,
            replica_set: None,
            database: None,
            user: "atlas-reader".into(),
            password_command: vec!["/bin/echo".into()],
            ssh_hops,
        }
    }

    fn bastion_hop() -> SshHop {
        SshHop {
            label: "bastion-alias".into(),
            host: "bastion.example".into(),
            port: 2228,
            user: "jump".into(),
            auth: SshAuth::Agent,
            host_key: verified(vec![PathBuf::from("/home/me/.ssh/known_hosts")]),
        }
    }

    #[test]
    fn une_connexion_mongodb_sans_alias_ssh_n_a_pas_de_tunnel() {
        let params = mongo(
            MongoTransport::Srv {
                host: "cluster0.atlas.scarif.test".into(),
            },
            None,
        );
        assert_eq!(mongo_tunnel(&params, None), None);
    }

    #[test]
    fn le_tunnel_mongodb_vise_l_hote_d_amorce_avec_le_known_hosts_configure() {
        let configured = PathBuf::from("/etc/scarif/known_hosts");
        let params = mongo(
            MongoTransport::Seed {
                host: "pl-0.atlas.scarif.test".into(),
                port: 1025,
                tls: false,
            },
            Some(vec![bastion_hop()]),
        );
        let tunnel = mongo_tunnel(&params, Some(&configured)).expect("tunnel");
        assert_eq!(tunnel.remote_host, "pl-0.atlas.scarif.test");
        assert_eq!(tunnel.remote_port, 1025);
        assert_eq!(tunnel.hops.len(), 1);
        assert_eq!(tunnel.hops[0].known_hosts, vec![configured]);
        assert!(tunnel.hops[0].verify_host_key);
    }

    #[test]
    fn le_tunnel_d_une_connexion_srv_vise_le_nom_srv_sur_le_port_par_defaut() {
        let params = mongo(
            MongoTransport::Srv {
                host: "cluster0.atlas.scarif.test".into(),
            },
            Some(vec![bastion_hop()]),
        );
        let tunnel = mongo_tunnel(&params, None).expect("tunnel");
        assert_eq!(tunnel.remote_host, "cluster0.atlas.scarif.test");
        assert_eq!(tunnel.remote_port, 27017);
    }

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
        let configured = PathBuf::from("/etc/scarif/known_hosts");
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
        let configured = PathBuf::from("/etc/scarif/known_hosts");
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
