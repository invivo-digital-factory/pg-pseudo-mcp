use crate::contexts::connections::domain::constants::{ENGINE_MONGODB, MONGODB_NOT_SUPPORTED};
use crate::contexts::connections::domain::errors::ConnectionError;
use crate::contexts::connections::domain::types::{ConnectParams, ConnectionStatus, Discovery};

pub fn resolve_connection(
    discovery: &Discovery,
    name: &str,
) -> Result<ConnectParams, ConnectionError> {
    let Some(entry) = discovery
        .connections
        .iter()
        .find(|entry| entry.name == name)
    else {
        let available = discovery
            .connections
            .iter()
            .filter(|entry| entry.status == ConnectionStatus::Available)
            .map(|entry| entry.name.clone())
            .collect();
        return Err(ConnectionError::Unknown {
            name: name.to_string(),
            available,
        });
    };
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
