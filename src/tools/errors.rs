use rmcp::model::{CallToolResult, ContentBlock};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    SqlError,
    RefusedReadOnly,
    Timeout,
    UnknownConnection,
    UnknownTable,
    UnknownColumn,
    ConnectionUnavailable,
    WithheldFailClosed,
    NotImplemented,
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
            message: message.into(),
            hint: None,
            retryable: false,
            sqlstate: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
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

    pub fn not_implemented(tool: &str) -> Self {
        Self::new(
            ErrorCategory::NotImplemented,
            "not_implemented",
            format!("{tool} is not available in this build"),
        )
    }

    pub fn into_call_result(self) -> CallToolResult {
        let body = serde_json::json!({ "error": self });
        CallToolResult::error(vec![ContentBlock::text(body.to_string())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_category_in_snake_case_and_omits_empty_fields() {
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
}
