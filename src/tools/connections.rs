use rmcp::model::CallToolResult;

use crate::state::AppConfig;
use crate::tools::errors::ToolError;

pub async fn list_connections(_config: &AppConfig) -> CallToolResult {
    ToolError::not_implemented("list_connections").into_call_result()
}
