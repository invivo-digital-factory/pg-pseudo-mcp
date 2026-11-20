use crate::contexts::connections::domain::types::DbMessage;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SessionError {
    #[error("connection failed: {0}")]
    Connect(String),
    #[error("connection failed: {0}")]
    Authentication(String),
    #[error("statement timed out")]
    Timeout,
    #[error("{}", describe(.0))]
    Db(Box<DbMessage>),
    #[error("cannot decode result: {0}")]
    Decode(String),
}

impl SessionError {
    pub fn is_authentication_failure(&self) -> bool {
        matches!(self, Self::Authentication(_))
    }

    pub fn breaks_connection(&self) -> bool {
        matches!(self, Self::Connect(_) | Self::Authentication(_))
    }
}

fn describe(message: &DbMessage) -> String {
    match &message.sqlstate {
        Some(sqlstate) => format!("{} {}: {}", message.severity, sqlstate, message.message),
        None => format!("{}: {}", message.severity, message.message),
    }
}
