use crate::contexts::connections::domain::errors::{ConnectionError, SessionError};
use crate::tools::errors::{ErrorCategory, ToolError};

pub fn tool_error(
    error: ConnectionError,
    session: impl FnOnce(SessionError) -> ToolError,
) -> ToolError {
    match error {
        ConnectionError::Unknown { name, available } => {
            let hint = if available.is_empty() {
                "no connection is available; call list_connections".to_string()
            } else {
                format!("available connections: {}", available.join(", "))
            };
            ToolError::new(
                ErrorCategory::UnknownConnection,
                "unknown_connection",
                format!("unknown connection '{name}'"),
            )
            .with_hint(hint)
        }
        ConnectionError::Unsupported { reason } => ToolError::new(
            ErrorCategory::ConnectionUnavailable,
            "connection_unavailable",
            reason,
        )
        .with_hint("this connection cannot be used by this server; pick another one"),
        ConnectionError::Unavailable { reason } => ToolError::new(
            ErrorCategory::ConnectionUnavailable,
            "connection_unavailable",
            reason,
        )
        .with_hint("ask the developer to fix the connection in DBeaver"),
        ConnectionError::MissingParams => ToolError::new(
            ErrorCategory::Internal,
            "missing_connect_params",
            "connection has no resolved parameters",
        ),
        ConnectionError::ConnectFailed { connection, source } => ToolError::new(
            ErrorCategory::ConnectionUnavailable,
            "connection_failed",
            format!("could not connect to {connection}: {source}"),
        )
        .with_hint("check that the database is reachable from this machine, then retry")
        .retryable(),
        ConnectionError::Session(error) => session(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unused(_: SessionError) -> ToolError {
        unreachable!("no session error in these cases")
    }

    #[test]
    fn an_unknown_connection_lists_the_available_ones() {
        let error = tool_error(
            ConnectionError::Unknown {
                name: "Nope".into(),
                available: vec!["Shop".into()],
            },
            unused,
        );
        assert_eq!(error.code, "unknown_connection");
        assert_eq!(error.category, ErrorCategory::UnknownConnection);
        assert!(error.hint.unwrap().contains("Shop"));
    }

    #[test]
    fn an_unavailable_connection_asks_the_developer_to_fix_dbeaver() {
        let error = tool_error(
            ConnectionError::Unavailable {
                reason: "no saved password".into(),
            },
            unused,
        );
        assert_eq!(error.code, "connection_unavailable");
        assert_eq!(error.category, ErrorCategory::ConnectionUnavailable);
        assert!(error.message.contains("no saved password"));
        assert_eq!(
            error.hint.as_deref(),
            Some("ask the developer to fix the connection in DBeaver")
        );
    }
}
