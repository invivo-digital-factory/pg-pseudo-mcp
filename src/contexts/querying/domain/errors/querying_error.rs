use super::{DatabaseFailure, Refusal};
use crate::contexts::querying::domain::types::DbMessage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScrubbedText {
    Nothing,
    Message(DbMessage),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum QueryingError {
    #[error("pseudonym store unavailable: {reason}")]
    StoreUnavailable { reason: String },
    #[error("{0}")]
    Refused(Refusal),
    #[error("unknown connection '{name}'")]
    UnknownConnection {
        name: String,
        available: Vec<String>,
    },
    #[error("{reason}")]
    ConnectionUnsupported { reason: String },
    #[error("{reason}")]
    ConnectionUnavailable { reason: String },
    #[error("connection has no resolved parameters")]
    MissingConnectParams,
    #[error("could not connect to {connection}: {reason}")]
    ConnectFailed { connection: String, reason: String },
    #[error("the statement ran longer than {timeout_ms} ms")]
    StatementTimeout { timeout_ms: u64 },
    #[error("{message}")]
    SqlError {
        message: String,
        sqlstate: Option<String>,
        detail: Option<String>,
        hint: Option<String>,
    },
    #[error("{message}")]
    ConnectionLost { message: String },
    #[error("{message}")]
    ResultWithheld { message: String },
    #[error("table {table} disappeared while it was described")]
    TableDisappeared { table: String },
    #[error("{requested} exists in several schemas")]
    AmbiguousTable {
        requested: String,
        candidates: Vec<String>,
    },
    #[error("no table, view or materialized view named {requested}")]
    UnknownTable {
        requested: String,
        suggestions: Vec<String>,
    },
}

impl QueryingError {
    pub fn from_failure(failure: DatabaseFailure, timeout_ms: u64, scrubbed: ScrubbedText) -> Self {
        match failure {
            DatabaseFailure::UnknownConnection { name, available } => {
                Self::UnknownConnection { name, available }
            }
            DatabaseFailure::Unsupported { reason } => Self::ConnectionUnsupported { reason },
            DatabaseFailure::Unavailable { reason } => Self::ConnectionUnavailable { reason },
            DatabaseFailure::MissingParams => Self::MissingConnectParams,
            DatabaseFailure::ConnectFailed { connection, reason } => Self::ConnectFailed {
                connection,
                reason: scrubbed_text(scrubbed, reason),
            },
            DatabaseFailure::Timeout => Self::StatementTimeout { timeout_ms },
            DatabaseFailure::Db(original) => {
                let message = match scrubbed {
                    ScrubbedText::Message(message) => message,
                    _ => *original,
                };
                Self::SqlError {
                    message: message.message,
                    sqlstate: message.sqlstate,
                    detail: message.detail,
                    hint: message.hint,
                }
            }
            DatabaseFailure::Connect(original) => Self::ConnectionLost {
                message: scrubbed_text(scrubbed, original),
            },
            DatabaseFailure::Decode(original) => Self::ResultWithheld {
                message: scrubbed_text(scrubbed, original),
            },
        }
    }
}

fn scrubbed_text(scrubbed: ScrubbedText, original: String) -> String {
    match scrubbed {
        ScrubbedText::Text(text) => text,
        _ => original,
    }
}
