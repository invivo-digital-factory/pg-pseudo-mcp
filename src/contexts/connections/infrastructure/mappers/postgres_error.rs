use tokio_postgres::error::{DbError, SqlState};

use crate::contexts::connections::domain::errors::SessionError;
use crate::contexts::connections::domain::types::DbMessage;

pub fn db_message_from_db_error(error: &DbError) -> DbMessage {
    DbMessage {
        severity: error.severity().to_string(),
        sqlstate: Some(error.code().code().to_string()),
        message: error.message().to_string(),
        detail: error.detail().map(str::to_string),
        hint: error.hint().map(str::to_string),
        where_: error.where_().map(str::to_string),
    }
}

pub fn session_error_from_postgres(error: &tokio_postgres::Error) -> SessionError {
    match error.as_db_error() {
        Some(db) if db.code() == &SqlState::QUERY_CANCELED => SessionError::Timeout,
        Some(db) => SessionError::Db(Box::new(db_message_from_db_error(db))),
        None => SessionError::Connect(error.to_string()),
    }
}
