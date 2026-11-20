use crate::contexts::connections::domain::errors::SessionError;
use crate::contexts::connections::domain::types::ConnectOptions;

const PLACEHOLDER: &str = "[redacted]";

pub fn redact_connection_identity(error: SessionError, options: &ConnectOptions) -> SessionError {
    let secrets = identifying_values(options);
    match error {
        SessionError::Connect(text) => SessionError::Connect(redact(&text, &secrets)),
        SessionError::Authentication(text) => SessionError::Authentication(redact(&text, &secrets)),
        other => other,
    }
}

fn identifying_values(options: &ConnectOptions) -> Vec<&str> {
    let mut values = vec![options.user.as_str(), options.host.as_str()];
    if let Some(tunnel) = &options.tunnel {
        values.extend([
            tunnel.host.as_str(),
            tunnel.user.as_str(),
            tunnel.remote_host.as_str(),
        ]);
    }
    values.retain(|value| !value.is_empty());
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    values
}

fn redact(text: &str, secrets: &[&str]) -> String {
    secrets.iter().fold(text.to_string(), |text, secret| {
        text.replace(secret, PLACEHOLDER)
    })
}

#[cfg(test)]
mod unit {
    use super::*;
    use crate::contexts::connections::domain::types::{SslMode, Tunnel, TunnelAuth};

    fn options(user: &str, host: &str, tunnel_host: Option<&str>) -> ConnectOptions {
        ConnectOptions {
            host: host.into(),
            port: 5432,
            database: "app".into(),
            user: user.into(),
            password: "pw".into(),
            sslmode: SslMode::Disable,
            tunnel: tunnel_host.map(|host| Tunnel {
                host: host.into(),
                port: 22,
                user: "jump".into(),
                auth: TunnelAuth::Agent,
                remote_host: host.into(),
                remote_port: 5432,
                verify_host_key: false,
                known_hosts: None,
            }),
        }
    }

    #[test]
    fn le_role_l_hote_et_l_hote_du_tunnel_sont_remplaces_dans_le_message() {
        let error = SessionError::Authentication(
            "SQLSTATE 28P01: password authentication failed for user \"pgpseudo\" via db.internal.example through bastion.corp".into(),
        );
        let redacted = redact_connection_identity(
            error,
            &options("pgpseudo", "db.internal.example", Some("bastion.corp")),
        );
        let text = redacted.to_string();
        assert!(text.contains("28P01"), "{text}");
        assert!(text.contains("password authentication failed"), "{text}");
        assert!(!text.contains("pgpseudo"), "{text}");
        assert!(!text.contains("db.internal.example"), "{text}");
        assert!(!text.contains("bastion.corp"), "{text}");
        assert!(redacted.is_authentication_failure());
    }

    fn tunnel_options() -> ConnectOptions {
        let mut options = options("pgpseudo", "db.internal.example", Some("bastion.corp"));
        if let Some(tunnel) = options.tunnel.as_mut() {
            tunnel.user = "etienne.bernoux".into();
            tunnel.remote_host = "pg-prod.private.example".into();
        }
        options
    }

    #[test]
    fn un_refus_d_authentification_ssh_ne_rend_pas_l_utilisateur_du_tunnel() {
        let error = SessionError::Connect(
            "ssh authentication rejected for etienne.bernoux at bastion.corp".into(),
        );
        let text = redact_connection_identity(error, &tunnel_options()).to_string();
        assert!(text.contains("ssh authentication rejected"), "{text}");
        assert!(!text.contains("etienne.bernoux"), "{text}");
        assert!(!text.contains("bastion.corp"), "{text}");
    }

    #[test]
    fn un_refus_de_redirection_du_bastion_ne_rend_pas_l_hote_distant() {
        let error =
            SessionError::Connect("bastion refused forward to pg-prod.private.example:5432".into());
        let text = redact_connection_identity(error, &tunnel_options()).to_string();
        assert!(text.contains("refused forward"), "{text}");
        assert!(!text.contains("pg-prod.private.example"), "{text}");
    }

    #[test]
    fn une_erreur_de_connexion_sans_identite_est_rendue_telle_quelle() {
        let error = SessionError::Connect("connection refused".into());
        assert_eq!(
            redact_connection_identity(error.clone(), &options("pgpseudo", "db", None)),
            error
        );
    }
}
