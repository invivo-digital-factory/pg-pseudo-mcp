use super::result_set_mapping::db_message;
use crate::contexts::connections::domain::errors::{ConnectionError, SessionError};
use crate::contexts::querying::domain::errors::DatabaseFailure;

pub fn database_failure(error: ConnectionError) -> DatabaseFailure {
    match error {
        ConnectionError::Unknown { name, available } => {
            DatabaseFailure::UnknownConnection { name, available }
        }
        ConnectionError::Unsupported { reason } => DatabaseFailure::Unsupported { reason },
        ConnectionError::Unavailable { reason } => DatabaseFailure::Unavailable { reason },
        ConnectionError::MissingParams => DatabaseFailure::MissingParams,
        ConnectionError::ConnectFailed { connection, source } => DatabaseFailure::ConnectFailed {
            connection,
            reason: source.to_string(),
        },
        ConnectionError::Session(error) => session_failure(error),
    }
}

fn session_failure(error: SessionError) -> DatabaseFailure {
    match error {
        SessionError::Timeout => DatabaseFailure::Timeout,
        SessionError::Db(message) => DatabaseFailure::Db(Box::new(db_message(*message))),
        SessionError::Connect(message) => DatabaseFailure::Connect(message),
        SessionError::Decode(message) => DatabaseFailure::Decode(message),
    }
}
