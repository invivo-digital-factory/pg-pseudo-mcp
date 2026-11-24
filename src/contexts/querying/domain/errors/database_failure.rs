use crate::contexts::querying::domain::types::DbMessage;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DatabaseFailure {
    #[error("unknown connection '{name}'")]
    UnknownConnection {
        name: String,
        available: Vec<String>,
    },
    #[error("{reason}")]
    Unsupported { reason: String },
    #[error("{reason}")]
    Unavailable { reason: String },
    #[error("connection has no resolved parameters")]
    MissingParams,
    #[error("could not connect to {connection}: {reason}")]
    ConnectFailed { connection: String, reason: String },
    #[error("statement timed out")]
    Timeout,
    #[error("{}", .0.message)]
    Db(Box<DbMessage>),
    #[error("connection failed: {0}")]
    Connect(String),
    #[error("cannot decode result: {0}")]
    Decode(String),
    #[error("no table, view or materialized view named {requested}")]
    UnknownTable { requested: String },
}
