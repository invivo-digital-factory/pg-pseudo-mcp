use crate::contexts::connections::domain::constants::{
    ENGINE_MONGODB, MONGODB_NOT_SUPPORTED, NOT_A_MONGODB_CONNECTION,
};
use crate::contexts::connections::domain::errors::ConnectionError;
use crate::contexts::connections::domain::types::{
    ConnectParams, ConnectionEntry, ConnectionStatus, Discovery, MongoConnectParams,
};

pub fn resolve_connection(
    discovery: &Discovery,
    name: &str,
) -> Result<ConnectParams, ConnectionError> {
    let entry = find_entry(discovery, name)?;
    match (&entry.status, &entry.params) {
        (ConnectionStatus::Available, Some(params)) => Ok(params.clone()),
        (ConnectionStatus::Unsupported { reason }, _) => Err(ConnectionError::Unsupported {
            reason: reason.clone(),
        }),
        (ConnectionStatus::Unavailable { reason }, _) => Err(ConnectionError::Unavailable {
            reason: reason.clone(),
        }),
        (ConnectionStatus::Available, None) if entry.engine == ENGINE_MONGODB => {
            Err(ConnectionError::Unsupported {
                reason: MONGODB_NOT_SUPPORTED.to_string(),
            })
        }
        (ConnectionStatus::Available, None) => Err(ConnectionError::MissingParams),
    }
}

pub fn resolve_mongo_connection(
    discovery: &Discovery,
    name: &str,
) -> Result<MongoConnectParams, ConnectionError> {
    let entry = find_entry(discovery, name)?;
    match (&entry.status, &entry.mongo) {
        (ConnectionStatus::Available, Some(params)) => Ok(params.clone()),
        (ConnectionStatus::Unsupported { reason }, _) => Err(ConnectionError::Unsupported {
            reason: reason.clone(),
        }),
        (ConnectionStatus::Unavailable { reason }, _) => Err(ConnectionError::Unavailable {
            reason: reason.clone(),
        }),
        (ConnectionStatus::Available, None) => Err(ConnectionError::Unsupported {
            reason: NOT_A_MONGODB_CONNECTION.to_string(),
        }),
    }
}

fn find_entry<'a>(
    discovery: &'a Discovery,
    name: &str,
) -> Result<&'a ConnectionEntry, ConnectionError> {
    discovery
        .connections
        .iter()
        .find(|entry| entry.name == name)
        .ok_or_else(|| ConnectionError::Unknown {
            name: name.to_string(),
            available: discovery
                .connections
                .iter()
                .filter(|entry| entry.status == ConnectionStatus::Available)
                .map(|entry| entry.name.clone())
                .collect(),
        })
}

#[cfg(test)]
mod unit {
    use super::*;
    use crate::contexts::connections::domain::types::{ConnectionSource, MongoTransport};

    fn mongo_params() -> MongoConnectParams {
        MongoConnectParams {
            transport: MongoTransport::Seed {
                host: "pl-0.atlas.scarif.test".into(),
                port: 1024,
                tls: false,
            },
            replica_set: Some("rs0".into()),
            database: Some("shop".into()),
            user: "atlas-reader".into(),
            password_command: vec!["/bin/echo".into(), "pw".into()],
            ssh_hops: None,
        }
    }

    fn entry(
        name: &str,
        status: ConnectionStatus,
        mongo: Option<MongoConnectParams>,
    ) -> ConnectionEntry {
        ConnectionEntry {
            id: name.into(),
            name: name.into(),
            declared_name: name.into(),
            source: ConnectionSource::File,
            folder: None,
            project: "General".into(),
            engine: if mongo.is_some() {
                ENGINE_MONGODB.into()
            } else {
                "postgresql".into()
            },
            connection_type: "dev".into(),
            database: None,
            status,
            params: None,
            mongo,
        }
    }

    fn discovery(entries: Vec<ConnectionEntry>) -> Discovery {
        Discovery {
            connections: entries,
            hint: None,
        }
    }

    #[test]
    fn une_connexion_mongodb_disponible_rend_ses_parametres() {
        let found = discovery(vec![entry(
            "atlas",
            ConnectionStatus::Available,
            Some(mongo_params()),
        )]);
        assert_eq!(
            resolve_mongo_connection(&found, "atlas").expect("params"),
            mongo_params()
        );
    }

    #[test]
    fn une_connexion_mongodb_reste_refusee_par_le_resolveur_postgresql() {
        let found = discovery(vec![entry(
            "atlas",
            ConnectionStatus::Available,
            Some(mongo_params()),
        )]);
        assert_eq!(
            resolve_connection(&found, "atlas").expect_err("interim refusal"),
            ConnectionError::Unsupported {
                reason: MONGODB_NOT_SUPPORTED.to_string()
            }
        );
    }

    #[test]
    fn une_connexion_postgresql_n_est_pas_ouverte_comme_mongodb() {
        let found = discovery(vec![entry("pg", ConnectionStatus::Available, None)]);
        assert_eq!(
            resolve_mongo_connection(&found, "pg").expect_err("not mongodb"),
            ConnectionError::Unsupported {
                reason: NOT_A_MONGODB_CONNECTION.to_string()
            }
        );
    }

    #[test]
    fn une_connexion_mongodb_indisponible_rend_sa_raison() {
        let reason = "auth must be command".to_string();
        let found = discovery(vec![entry(
            "atlas",
            ConnectionStatus::Unavailable {
                reason: reason.clone(),
            },
            None,
        )]);
        assert_eq!(
            resolve_mongo_connection(&found, "atlas").expect_err("unavailable"),
            ConnectionError::Unavailable { reason }
        );
    }

    #[test]
    fn une_connexion_inconnue_liste_les_connexions_disponibles() {
        let found = discovery(vec![entry(
            "atlas",
            ConnectionStatus::Available,
            Some(mongo_params()),
        )]);
        assert_eq!(
            resolve_mongo_connection(&found, "absent").expect_err("unknown"),
            ConnectionError::Unknown {
                name: "absent".into(),
                available: vec!["atlas".into()]
            }
        );
    }
}
