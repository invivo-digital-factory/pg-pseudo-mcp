use super::session_error::SessionError;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ConnectionError {
    #[error("unknown connection '{name}'")]
    Unknown {
        name: String,
        available: Vec<String>,
    },
    #[error("{reason}")]
    Unavailable { reason: String },
    #[error("{reason}")]
    Unsupported { reason: String },
    #[error("connection has no resolved parameters")]
    MissingParams,
    #[error("could not connect to {connection}: {source}")]
    ConnectFailed {
        connection: String,
        source: SessionError,
    },
    #[error("no collection or view named {requested}")]
    UnknownCollection { requested: String },
    #[error(transparent)]
    Session(#[from] SessionError),
}

impl ConnectionError {
    pub fn discovery_interrupted() -> Self {
        Self::Unavailable {
            reason: "connection discovery was interrupted".to_string(),
        }
    }
}
