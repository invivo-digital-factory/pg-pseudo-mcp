use super::tool_error::{ErrorCategory, ToolError};
use crate::contexts::querying::domain::errors::QueryingError;

pub fn tool_error(error: QueryingError) -> ToolError {
    match error {
        QueryingError::StoreUnavailable { reason } => ToolError::new(
            ErrorCategory::WithheldFailClosed,
            "pseudonym_store_unavailable",
            reason,
        )
        .with_hint("ask the developer to run scarif-database-mcp outside the agent to repair the pseudonym store"),
        QueryingError::Refused(refusal) => {
            let error = ToolError::new(
                ErrorCategory::RefusedReadOnly,
                refusal.code,
                refusal.message,
            );
            match refusal.hint {
                Some(hint) => error.with_hint(hint),
                None => error,
            }
        }
        QueryingError::UnknownConnection { name, available } => {
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
        QueryingError::ConnectionUnsupported { reason } => ToolError::new(
            ErrorCategory::ConnectionUnavailable,
            "connection_unavailable",
            reason,
        )
        .with_hint("this connection cannot be used by this server; pick another one"),
        QueryingError::ConnectionUnavailable { reason } => ToolError::new(
            ErrorCategory::ConnectionUnavailable,
            "connection_unavailable",
            reason,
        )
        .with_hint("ask the developer to fix the connection definition (connection file or DBeaver)"),
        QueryingError::MissingConnectParams => ToolError::new(
            ErrorCategory::Internal,
            "missing_connect_params",
            "connection has no resolved parameters",
        ),
        QueryingError::ConnectFailed { connection, reason } => ToolError::new(
            ErrorCategory::ConnectionUnavailable,
            "connection_failed",
            format!("could not connect to {connection}: {reason}"),
        )
        .with_hint("check that the database is reachable from this machine, then retry")
        .retryable(),
        QueryingError::StatementTimeout { timeout_ms } => ToolError::new(
            ErrorCategory::Timeout,
            "statement_timeout",
            format!("the statement ran longer than {timeout_ms} ms"),
        )
        .with_hint("narrow the filter, check indexes with describe_table, or inspect the plan with explain")
        .retryable(),
        QueryingError::SqlError {
            message,
            sqlstate,
            detail,
            hint,
        } => {
            let mut error = ToolError::new(ErrorCategory::SqlError, "sql_error", message);
            if let Some(sqlstate) = sqlstate {
                error = error.with_sqlstate(sqlstate);
            }
            match (detail, hint) {
                (Some(detail), Some(hint)) => error.with_hint(format!("{detail} {hint}")),
                (Some(text), None) | (None, Some(text)) => error.with_hint(text),
                (None, None) => error,
            }
        }
        QueryingError::DocumentServerError { message, code_name } => {
            let error = ToolError::new(ErrorCategory::MongodbError, "mongodb_error", message)
                .with_hint("the MongoDB server refused the request; values it quoted are pseudonymized");
            match code_name {
                Some(code_name) => error.with_code_name(code_name),
                None => error,
            }
        }
        QueryingError::ConnectionLost { message } => ToolError::new(
            ErrorCategory::ConnectionUnavailable,
            "connection_lost",
            message,
        )
        .with_hint("retry; the server reconnects on the next call")
        .retryable(),
        QueryingError::ResultWithheld { message } => ToolError::new(
            ErrorCategory::WithheldFailClosed,
            "withheld_fail_closed",
            format!("a value could not be decoded safely and the result was withheld: {message}"),
        )
        .with_hint("select fewer columns or cast the problematic column to text"),
        QueryingError::TableDisappeared { table } => ToolError::new(
            ErrorCategory::UnknownTable,
            "unknown_table",
            format!("table {table} disappeared while it was described"),
        )
        .retryable(),
        QueryingError::AmbiguousTable {
            requested,
            candidates,
        } => ToolError::new(
            ErrorCategory::UnknownTable,
            "ambiguous_table",
            format!(
                "{requested} exists in several schemas: {}",
                candidates.join(", ")
            ),
        )
        .with_hint("qualify the table with its schema (schema.table)"),
        QueryingError::UnknownTable {
            requested,
            suggestions,
        } => {
            let hint = if suggestions.is_empty() {
                "use search_schema or list_tables to find the table".to_string()
            } else {
                format!("closest names: {}", suggestions.join(", "))
            };
            ToolError::new(
                ErrorCategory::UnknownTable,
                "unknown_table",
                format!("no table, view or materialized view named {requested}"),
            )
            .with_hint(hint)
        }
    }
}

pub fn wrong_engine(message: &str) -> ToolError {
    ToolError::new(
        ErrorCategory::ConnectionUnavailable,
        "wrong_engine",
        message,
    )
    .with_hint("list_connections shows the engine of each connection")
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn une_erreur_mongodb_porte_son_code_name_et_jamais_un_sqlstate() {
        let error = tool_error(QueryingError::DocumentServerError {
            message: "Failed to parse number '<value id=1>'".into(),
            code_name: Some("ConversionFailure".into()),
        });
        let json = serde_json::to_value(&error).unwrap();
        assert_eq!(json["code"], "mongodb_error");
        assert_eq!(json["category"], "mongodb_error");
        assert_eq!(json["code_name"], "ConversionFailure");
        assert!(json.get("sqlstate").is_none(), "{json}");
    }

    #[test]
    fn une_connexion_inconnue_liste_les_connexions_disponibles() {
        let error = tool_error(QueryingError::UnknownConnection {
            name: "Nope".into(),
            available: vec!["Shop".into()],
        });
        assert_eq!(error.code, "unknown_connection");
        assert_eq!(error.category, ErrorCategory::UnknownConnection);
        assert!(error.hint.unwrap().contains("Shop"));
    }

    #[test]
    fn une_connexion_indisponible_demande_de_corriger_dbeaver() {
        let error = tool_error(QueryingError::ConnectionUnavailable {
            reason: "no saved password".into(),
        });
        assert_eq!(error.code, "connection_unavailable");
        assert_eq!(error.category, ErrorCategory::ConnectionUnavailable);
        assert!(error.message.contains("no saved password"));
        assert_eq!(
            error.hint.as_deref(),
            Some("ask the developer to fix the connection definition (connection file or DBeaver)")
        );
    }
}
