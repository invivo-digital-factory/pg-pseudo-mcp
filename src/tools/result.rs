use rmcp::model::{CallToolResult, ContentBlock};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Warning {
    pub code: String,
    pub message: String,
}

impl Warning {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TruncatedBy {
    Rows,
    Bytes,
    Cell,
    Timeout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Truncation {
    pub by: TruncatedBy,
    pub limit: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
    pub hint: String,
}

pub fn json_result<T: Serialize>(value: &T) -> CallToolResult {
    match serde_json::to_string(value) {
        Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
        Err(error) => crate::tools::errors::ToolError::new(
            crate::tools::errors::ErrorCategory::Internal,
            "serialization_failed",
            error.to_string(),
        )
        .into_call_result(),
    }
}
