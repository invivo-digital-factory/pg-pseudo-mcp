use std::path::Path;

use crate::contexts::connections::domain::types::{
    ConnectOptions, ConnectParams, SshAuth, Tunnel, TunnelAuth,
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
            host: tunnel.host.clone(),
            port: tunnel.port,
            user: tunnel.user.clone(),
            auth: match &tunnel.auth {
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
            remote_host: tunnel.remote_host.clone(),
            remote_port: tunnel.remote_port,
            verify_host_key: tunnel.verify_host_key,
            known_hosts: known_hosts.map(Path::to_path_buf),
        }),
    }
}

#[cfg(test)]
mod unit {
    use std::path::PathBuf;

    use super::*;
    use crate::contexts::connections::domain::types::{Secret, SslMode, TunnelParams};

    fn tunneled(auth: SshAuth) -> ConnectParams {
        ConnectParams {
            host: "db.internal".into(),
            port: 5432,
            database: "app".into(),
            user: "reader".into(),
            password: Secret::new("db-pass"),
            sslmode: SslMode::Require,
            tunnel: Some(TunnelParams {
                host: "bastion.example".into(),
                port: 2222,
                user: "jump".into(),
                auth,
                remote_host: "db.internal".into(),
                remote_port: 6432,
                verify_host_key: false,
            }),
        }
    }

    #[test]
    fn un_tunnel_ssh_dbeaver_devient_un_tunnel_pg_avec_le_known_hosts_configure() {
        let known_hosts = PathBuf::from("/etc/pg-pseudo/known_hosts");
        let options = connect_options(
            &tunneled(SshAuth::PublicKey {
                key_path: PathBuf::from("/keys/id_ed25519"),
                passphrase: Some(Secret::new("phrase")),
            }),
            Some(&known_hosts),
        );
        assert_eq!(
            options.tunnel,
            Some(Tunnel {
                host: "bastion.example".into(),
                port: 2222,
                user: "jump".into(),
                auth: TunnelAuth::PublicKey {
                    key_path: PathBuf::from("/keys/id_ed25519"),
                    passphrase: Some("phrase".into()),
                },
                remote_host: "db.internal".into(),
                remote_port: 6432,
                verify_host_key: false,
                known_hosts: Some(PathBuf::from("/etc/pg-pseudo/known_hosts")),
            })
        );
        let password = connect_options(&tunneled(SshAuth::Password(Secret::new("ssh-pass"))), None);
        let tunnel = password.tunnel.expect("tunnel");
        assert_eq!(tunnel.auth, TunnelAuth::Password("ssh-pass".into()));
        assert_eq!(tunnel.known_hosts, None);
        let agent = connect_options(&tunneled(SshAuth::Agent), None);
        assert_eq!(agent.tunnel.expect("tunnel").auth, TunnelAuth::Agent);
    }
}
