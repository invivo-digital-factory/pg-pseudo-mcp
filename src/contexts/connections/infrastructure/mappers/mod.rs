mod connect_error_redaction;
mod mongo_error;
mod postgres_error;

pub use connect_error_redaction::redact_connection_identity;
pub use mongo_error::{
    AUTHENTICATION_REFUSED, MongoRedaction, PROXY_CHANNEL_REFUSED, connection_error_from_mongo,
    is_authentication_failure, redacted_connect_failure,
};

pub use postgres_error::{
    db_message_from_db_error, describe_postgres_error, session_error_from_handshake,
    session_error_from_postgres,
};
