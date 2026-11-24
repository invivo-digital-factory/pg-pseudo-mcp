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
        ConnectionError::UnknownCollection { requested } => {
            DatabaseFailure::UnknownTable { requested }
        }
        ConnectionError::Session(error) => session_failure(error),
    }
}

fn session_failure(error: SessionError) -> DatabaseFailure {
    match error {
        SessionError::Timeout => DatabaseFailure::Timeout,
        SessionError::Db(message) => DatabaseFailure::Db(Box::new(db_message(*message))),
        SessionError::Connect(message) | SessionError::Authentication(message) => {
            DatabaseFailure::Connect(message)
        }
        SessionError::Decode(message) => DatabaseFailure::Decode(message),
    }
}

#[cfg(test)]
mod unit {
    use super::*;
    use crate::contexts::querying::domain::errors::{QueryingError, ScrubbedText};

    #[test]
    fn une_collection_absente_rend_le_libelle_de_la_table_inconnue_de_postgresql() {
        let failure = database_failure(ConnectionError::UnknownCollection {
            requested: "ghosts".into(),
        });
        assert_eq!(
            failure,
            DatabaseFailure::UnknownTable {
                requested: "ghosts".into(),
            }
        );
        let error = QueryingError::from_failure(failure, 1_000, ScrubbedText::Nothing);
        assert_eq!(
            error.to_string(),
            "no table, view or materialized view named ghosts"
        );
    }
}
