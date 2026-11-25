use rmcp::model::{CallToolResult, ContentBlock};
use serde::Serialize;

use crate::common::redact_home;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    SqlError,
    MongodbError,
    RefusedReadOnly,
    Timeout,
    UnknownConnection,
    UnknownTable,
    ConnectionUnavailable,
    WithheldFailClosed,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolError {
    pub code: String,
    pub category: ErrorCategory,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sqlstate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_name: Option<String>,
}

impl ToolError {
    pub fn new(
        category: ErrorCategory,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            category,
            message: redact_home(&message.into()),
            hint: None,
            retryable: false,
            sqlstate: None,
            code_name: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(redact_home(&hint.into()));
        self
    }

    pub fn retryable(mut self) -> Self {
        self.retryable = true;
        self
    }

    pub fn with_sqlstate(mut self, sqlstate: impl Into<String>) -> Self {
        self.sqlstate = Some(sqlstate.into());
        self
    }

    pub fn with_code_name(mut self, code_name: impl Into<String>) -> Self {
        self.code_name = Some(code_name.into());
        self
    }

    pub fn into_call_result(self) -> CallToolResult {
        let body = serde_json::json!({ "error": self });
        CallToolResult::error(vec![ContentBlock::text(body.to_string())])
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn l_erreur_serialise_sa_categorie_en_snake_case_sans_champs_vides() {
        let error = ToolError::new(
            ErrorCategory::RefusedReadOnly,
            "dml",
            "DELETE is not allowed",
        )
        .with_hint("only SELECT, WITH and EXPLAIN are accepted");
        let json = serde_json::to_value(&error).unwrap();
        assert_eq!(json["category"], "refused_read_only");
        assert_eq!(json["hint"], "only SELECT, WITH and EXPLAIN are accepted");
        assert!(json.get("sqlstate").is_none());
        assert_eq!(json["retryable"], false);
    }

    #[test]
    fn le_repertoire_personnel_n_atteint_jamais_l_agent() {
        let home = std::env::var("HOME").unwrap();
        let error = ToolError::new(
            ErrorCategory::ConnectionUnavailable,
            "connection_failed",
            format!("SSH key file not found: {home}/.ssh/id_test"),
        )
        .with_hint(format!("check {home}/.ssh/known_hosts"));
        assert_eq!(error.message, "SSH key file not found: ~/.ssh/id_test");
        assert_eq!(error.hint.as_deref(), Some("check ~/.ssh/known_hosts"));
    }
}
