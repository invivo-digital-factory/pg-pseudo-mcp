use rmcp::model::{CallToolResult, ContentBlock};
use serde::Serialize;

use super::tool_error::{ErrorCategory, ToolError};

pub fn json_result<T: Serialize>(value: &T) -> CallToolResult {
    match serde_json::to_string(value) {
        Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
        Err(error) => ToolError::new(
            ErrorCategory::Internal,
            "serialization_failed",
            error.to_string(),
        )
        .into_call_result(),
    }
}
