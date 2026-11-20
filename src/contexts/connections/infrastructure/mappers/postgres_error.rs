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

pub fn session_error_from_handshake(error: &tokio_postgres::Error) -> SessionError {
    match error.as_db_error().map(DbError::code) {
        Some(code)
            if code == &SqlState::INVALID_PASSWORD
                || code == &SqlState::INVALID_AUTHORIZATION_SPECIFICATION =>
        {
            SessionError::Authentication(describe_postgres_error(error))
        }
        _ => SessionError::Connect(describe_postgres_error(error)),
    }
}

pub fn session_error_from_postgres(error: &tokio_postgres::Error) -> SessionError {
    match error.as_db_error() {
        Some(db) if db.code() == &SqlState::QUERY_CANCELED => SessionError::Timeout,
        Some(db) => SessionError::Db(Box::new(db_message_from_db_error(db))),
        None => SessionError::Connect(describe_postgres_error(error)),
    }
}

pub fn describe_postgres_error(error: &tokio_postgres::Error) -> String {
    if let Some(db) = error.as_db_error() {
        let mut text = format!("SQLSTATE {}: {}", db.code().code(), db.message());
        if let Some(detail) = db.detail() {
            text.push_str(&format!(" DETAIL: {detail}"));
        }
        if let Some(hint) = db.hint() {
            text.push_str(&format!(" HINT: {hint}"));
        }
        return text;
    }
    let mut text = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        text.push_str(&format!(": {cause}"));
        source = cause.source();
    }
    text
}
