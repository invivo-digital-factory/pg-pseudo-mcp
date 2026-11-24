use mongodb::error::ErrorKind;

use crate::contexts::connections::domain::errors::{ConnectionError, SessionError};
use crate::contexts::connections::domain::types::{DbMessage, MongoConnectParams, MongoTransport};
use crate::contexts::connections::infrastructure::data_sources::mongodb::MongoFailure;
use crate::contexts::connections::infrastructure::data_sources::socks::{
    ProxyCredentials, TargetPolicy,
};

pub const AUTHENTICATION_REFUSED: &str =
    "the MongoDB server refused the user name or the password of the password command";

const PLACEHOLDER: &str = "[redacted]";
const MAX_TIME_EXPIRED: i32 = 50;
const AUTHENTICATION_FAILED: i32 = 18;
const SERVER_ERROR: &str = "ERROR";

pub struct MongoRedaction {
    literals: Vec<String>,
    members: Option<TargetPolicy>,
}

enum FailureClass {
    Authentication,
    Timeout,
    Refused { code_name: String, message: String },
    Unreachable(String),
    Undecodable(String),
    InvalidRequest(String),
}

impl MongoRedaction {
    pub fn new(
        params: &MongoConnectParams,
        members: Option<TargetPolicy>,
        proxy: Option<&ProxyCredentials>,
    ) -> Self {
        let host = match &params.transport {
            MongoTransport::Srv { host } | MongoTransport::Seed { host, .. } => host,
        };
        let mut literals = vec![host.clone(), params.user.clone()];
        for hop in params.ssh_hops.iter().flatten() {
            literals.extend([hop.host.clone(), hop.user.clone()]);
        }
        if let Some(proxy) = proxy {
            literals.extend([proxy.username().to_string(), proxy.password().to_string()]);
        }
        literals.retain(|literal| !literal.is_empty());
        literals.sort_by_key(|literal| std::cmp::Reverse(literal.len()));
        literals.dedup();
        Self { literals, members }
    }

    pub fn redact(&self, text: &str) -> String {
        let text = self
            .literals
            .iter()
            .fold(text.to_string(), |text, literal| {
                text.replace(literal.as_str(), PLACEHOLDER)
            });
        match &self.members {
            Some(members) => mask_member_names(&text, members),
            None => text,
        }
    }
}

fn mask_member_names(text: &str, members: &TargetPolicy) -> String {
    let mut masked = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(is_name_char) {
        masked.push_str(&rest[..start]);
        let tail = &rest[start..];
        let end = tail.find(|c: char| !is_name_char(c)).unwrap_or(tail.len());
        let token = &tail[..end];
        let name = token.trim_end_matches('.');
        if members.admit(name, 1).is_some() {
            masked.push_str(PLACEHOLDER);
            masked.push_str(&token[name.len()..]);
        } else {
            masked.push_str(token);
        }
        rest = &tail[end..];
    }
    masked.push_str(rest);
    masked
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.' || c == '-'
}

fn classify(failure: MongoFailure) -> FailureClass {
    let error = match failure {
        MongoFailure::InvalidRequest(reason) => return FailureClass::InvalidRequest(reason),
        MongoFailure::Driver(error) => error,
    };
    match error.kind.as_ref() {
        ErrorKind::Authentication { .. } => FailureClass::Authentication,
        ErrorKind::Command(command) if command.code == AUTHENTICATION_FAILED => {
            FailureClass::Authentication
        }
        ErrorKind::Command(command) if command.code == MAX_TIME_EXPIRED => FailureClass::Timeout,
        ErrorKind::Command(command) => FailureClass::Refused {
            code_name: if command.code_name.is_empty() {
                command.code.to_string()
            } else {
                command.code_name.clone()
            },
            message: command.message.clone(),
        },
        kind @ (ErrorKind::BsonDeserialization(_)
        | ErrorKind::BsonSerialization(_)
        | ErrorKind::InvalidResponse { .. }) => FailureClass::Undecodable(kind.to_string()),
        kind => FailureClass::Unreachable(kind.to_string()),
    }
}

fn connection_error_from_class(
    connection: &str,
    class: FailureClass,
    redaction: &MongoRedaction,
) -> ConnectionError {
    let connect_failed = |source| ConnectionError::ConnectFailed {
        connection: connection.to_string(),
        source,
    };
    match class {
        FailureClass::Authentication => connect_failed(SessionError::Authentication(
            AUTHENTICATION_REFUSED.to_string(),
        )),
        FailureClass::Unreachable(text) => {
            connect_failed(SessionError::Connect(redaction.redact(&text)))
        }
        FailureClass::Timeout => ConnectionError::Session(SessionError::Timeout),
        FailureClass::Undecodable(text) => {
            ConnectionError::Session(SessionError::Decode(redaction.redact(&text)))
        }
        FailureClass::Refused { code_name, message } => {
            ConnectionError::Session(server_message(Some(code_name), redaction.redact(&message)))
        }
        FailureClass::InvalidRequest(reason) => {
            ConnectionError::Session(server_message(None, reason))
        }
    }
}

fn server_message(code_name: Option<String>, message: String) -> SessionError {
    SessionError::Db(Box::new(DbMessage {
        severity: SERVER_ERROR.to_string(),
        sqlstate: code_name,
        message,
        detail: None,
        hint: None,
        where_: None,
    }))
}

pub fn is_authentication_failure(failure: &MongoFailure) -> bool {
    match failure {
        MongoFailure::Driver(error) => match error.kind.as_ref() {
            ErrorKind::Authentication { .. } => true,
            ErrorKind::Command(command) => command.code == AUTHENTICATION_FAILED,
            _ => false,
        },
        MongoFailure::InvalidRequest(_) => false,
    }
}

pub fn connection_error_from_mongo(
    connection: &str,
    failure: MongoFailure,
    redaction: &MongoRedaction,
) -> ConnectionError {
    connection_error_from_class(connection, classify(failure), redaction)
}

pub fn redacted_connect_failure(
    connection: &str,
    error: SessionError,
    redaction: &MongoRedaction,
) -> ConnectionError {
    let source = match error {
        SessionError::Connect(text) => SessionError::Connect(redaction.redact(&text)),
        SessionError::Authentication(text) => SessionError::Authentication(redaction.redact(&text)),
        other => other,
    };
    ConnectionError::ConnectFailed {
        connection: connection.to_string(),
        source,
    }
}

#[cfg(test)]
mod unit {
    use mongodb::error::{CommandError, Error};

    use super::*;
    use crate::contexts::connections::domain::types::{HostKeyPolicy, SshAuth, SshHop};

    const MEMBER: &str = "pl-0.atlas.scarif.test";

    fn params() -> MongoConnectParams {
        MongoConnectParams {
            transport: MongoTransport::Seed {
                host: MEMBER.into(),
                port: 1024,
                tls: false,
            },
            replica_set: Some("rs0".into()),
            database: Some("shop".into()),
            user: "atlas-reader".into(),
            password_command: vec!["/bin/echo".into(), "pw".into()],
            ssh_hops: Some(vec![SshHop {
                label: "bastion-alias".into(),
                host: "bastion.corp.example".into(),
                port: 2228,
                user: "jump.user".into(),
                auth: SshAuth::Agent,
                host_key: HostKeyPolicy::Bypass,
            }]),
        }
    }

    fn redaction(proxy: Option<&ProxyCredentials>) -> MongoRedaction {
        MongoRedaction::new(
            &params(),
            Some(TargetPolicy::from_seed_host(MEMBER).expect("policy")),
            proxy,
        )
    }

    fn driver(kind: ErrorKind) -> MongoFailure {
        MongoFailure::Driver(Error::from(kind))
    }

    #[test]
    fn une_erreur_de_selection_ne_rend_ni_hote_ni_utilisateur_ni_identifiants_du_proxy() {
        let proxy = ProxyCredentials::generate().expect("credentials");
        let message = format!(
            "Server selection timeout: No available servers. Topology: {{ Type: ReplicaSetNoPrimary, \
             Servers: [ {{ Address: {MEMBER}:1024, Error: proxy refused }}, \
             {{ Address: pl-2.atlas.scarif.test:1026, Error: user atlas-reader via \
             bastion.corp.example as jump.user with {} {} }} ] }}",
            proxy.username(),
            proxy.password()
        );
        let error = connection_error_from_class(
            "atlas",
            FailureClass::Unreachable(format!("Server selection timeout: {message}")),
            &redaction(Some(&proxy)),
        );
        let ConnectionError::ConnectFailed { connection, source } = &error else {
            panic!("{error:?}");
        };
        assert_eq!(connection, "atlas");
        assert!(matches!(source, SessionError::Connect(_)), "{source:?}");
        let text = error.to_string();
        assert!(text.contains("Server selection timeout"), "{text}");
        assert!(
            text.contains(":1024") && text.contains("[redacted]"),
            "{text}"
        );
        for secret in [
            "pl-0.atlas.scarif.test",
            "pl-2.atlas.scarif.test",
            "atlas-reader",
            "bastion.corp.example",
            "jump.user",
            proxy.username(),
            proxy.password(),
        ] {
            assert!(!text.contains(secret), "{secret} in {text}");
        }
    }

    #[test]
    fn un_refus_d_authentification_rend_un_motif_fixe() {
        let error =
            connection_error_from_class("atlas", FailureClass::Authentication, &redaction(None));
        assert_eq!(
            error,
            ConnectionError::ConnectFailed {
                connection: "atlas".into(),
                source: SessionError::Authentication(AUTHENTICATION_REFUSED.into()),
            }
        );
    }

    #[test]
    fn un_refus_du_serveur_code_18_est_un_echec_d_authentification() {
        let failure = driver(ErrorKind::Command(command_error(
            18,
            "AuthenticationFailed",
            "Authentication failed.",
        )));
        assert!(is_authentication_failure(&failure));
        assert_eq!(
            connection_error_from_mongo("atlas", failure, &redaction(None)),
            ConnectionError::ConnectFailed {
                connection: "atlas".into(),
                source: SessionError::Authentication(AUTHENTICATION_REFUSED.into()),
            }
        );
    }

    #[test]
    fn une_erreur_d_entree_sortie_est_un_echec_de_connexion_expurge() {
        let failure = driver(ErrorKind::Io(std::sync::Arc::new(std::io::Error::other(
            "connection reset by pl-1.atlas.scarif.test:1025",
        ))));
        assert!(!is_authentication_failure(&failure));
        let error = connection_error_from_mongo("atlas", failure, &redaction(None));
        let ConnectionError::ConnectFailed { source, .. } = &error else {
            panic!("{error:?}");
        };
        assert_eq!(
            source,
            &SessionError::Connect("I/O error: connection reset by [redacted]:1025".into())
        );
    }

    fn command_error(code: i32, code_name: &str, message: &str) -> CommandError {
        serde_json::from_value(serde_json::json!({
            "code": code,
            "codeName": code_name,
            "errmsg": message,
        }))
        .expect("command error")
    }

    #[test]
    fn une_erreur_de_commande_rend_son_code_et_son_message_expurge() {
        let failure = driver(ErrorKind::Command(command_error(
            2,
            "BadValue",
            "unknown operator: $foo on pl-0.atlas.scarif.test",
        )));
        assert!(!is_authentication_failure(&failure));
        let error = connection_error_from_mongo("atlas", failure, &redaction(None));
        let ConnectionError::Session(SessionError::Db(message)) = &error else {
            panic!("{error:?}");
        };
        assert_eq!(message.sqlstate.as_deref(), Some("BadValue"));
        assert_eq!(message.message, "unknown operator: $foo on [redacted]");
    }

    #[test]
    fn un_depassement_de_max_time_ms_est_un_delai_depasse() {
        let failure = driver(ErrorKind::Command(command_error(
            50,
            "MaxTimeMSExpired",
            "operation exceeded time limit",
        )));
        assert_eq!(
            connection_error_from_mongo("atlas", failure, &redaction(None)),
            ConnectionError::Session(SessionError::Timeout)
        );
    }

    #[test]
    fn une_requete_invalide_rend_son_motif_fixe() {
        let error = connection_error_from_mongo(
            "atlas",
            MongoFailure::InvalidRequest("the filter is not valid Extended JSON".into()),
            &redaction(None),
        );
        let ConnectionError::Session(SessionError::Db(message)) = &error else {
            panic!("{error:?}");
        };
        assert_eq!(message.message, "the filter is not valid Extended JSON");
        assert_eq!(message.sqlstate, None);
    }

    #[test]
    fn un_echec_du_lien_ssh_est_expurge_de_l_hote_et_de_l_utilisateur_du_bastion() {
        let error = redacted_connect_failure(
            "atlas",
            SessionError::Connect(
                "SSH server bastion.corp.example:2228 refused jump.user to pl-0.atlas.scarif.test:1024".into(),
            ),
            &redaction(None),
        );
        let text = error.to_string();
        assert!(text.contains("refused"), "{text}");
        for secret in ["bastion.corp.example", "jump.user", MEMBER] {
            assert!(!text.contains(secret), "{secret} in {text}");
        }
    }

    #[test]
    fn un_nom_hors_du_domaine_des_membres_n_est_pas_masque() {
        let text = redaction(None).redact("Kind: I/O error at scarif.test and docs.mongodb.com.");
        assert_eq!(text, "Kind: I/O error at scarif.test and docs.mongodb.com.");
    }
}
