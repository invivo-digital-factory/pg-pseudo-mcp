use tokio_postgres::error::{DbError, SqlState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbMessage {
    pub severity: String,
    pub sqlstate: Option<String>,
    pub message: String,
    pub detail: Option<String>,
    pub hint: Option<String>,
    pub where_: Option<String>,
}

impl DbMessage {
    pub(crate) fn from_db_error(error: &DbError) -> Self {
        Self {
            severity: error.severity().to_string(),
            sqlstate: Some(error.code().code().to_string()),
            message: error.message().to_string(),
            detail: error.detail().map(str::to_string),
            hint: error.hint().map(str::to_string),
            where_: error.where_().map(str::to_string),
        }
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PgError {
    #[error("connection failed: {0}")]
    Connect(String),
    #[error("statement timed out")]
    Timeout,
    #[error("{}", describe(.0))]
    Db(Box<DbMessage>),
    #[error("cannot decode result: {0}")]
    Decode(String),
}

impl PgError {
    pub(crate) fn from_postgres(error: &tokio_postgres::Error) -> Self {
        match error.as_db_error() {
            Some(db) if db.code() == &SqlState::QUERY_CANCELED => Self::Timeout,
            Some(db) => Self::Db(Box::new(DbMessage::from_db_error(db))),
            None => Self::Connect(error.to_string()),
        }
    }

    pub(crate) fn breaks_connection(&self) -> bool {
        matches!(self, Self::Connect(_))
    }
}

fn describe(message: &DbMessage) -> String {
    match &message.sqlstate {
        Some(sqlstate) => format!("{} {}: {}", message.severity, sqlstate, message.message),
        None => format!("{}: {}", message.severity, message.message),
    }
}
