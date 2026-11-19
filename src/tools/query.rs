use rmcp::model::CallToolResult;
use rmcp::schemars;
use serde::Deserialize;

use crate::state::AppConfig;
use crate::tools::errors::ToolError;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct QueryArgs {
    #[schemars(description = "Connection name as returned by list_connections")]
    pub connection: String,
    #[schemars(description = "One read-only SQL statement")]
    pub sql: String,
    #[schemars(description = "Maximum rows returned (default 50, maximum 500)")]
    pub max_rows: Option<u64>,
    #[schemars(description = "Statement timeout in milliseconds (default 10000, maximum 60000)")]
    pub timeout_ms: Option<u64>,
}

pub async fn query(_config: &AppConfig, _args: QueryArgs) -> CallToolResult {
    ToolError::not_implemented("query").into_call_result()
}
