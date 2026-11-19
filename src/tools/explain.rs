use rmcp::model::CallToolResult;
use rmcp::schemars;
use serde::Deserialize;

use crate::state::AppConfig;
use crate::tools::errors::ToolError;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExplainArgs {
    #[schemars(description = "Connection name as returned by list_connections")]
    pub connection: String,
    #[schemars(description = "One read-only SQL statement to explain")]
    pub sql: String,
}

pub async fn explain(_config: &AppConfig, _args: ExplainArgs) -> CallToolResult {
    ToolError::not_implemented("explain").into_call_result()
}
