use rmcp::model::CallToolResult;
use rmcp::schemars;
use serde::Deserialize;

use crate::state::AppState;
use crate::tools::errors::ToolError;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchSchemaArgs {
    #[schemars(description = "Connection name as returned by list_connections")]
    pub connection: String,
    #[schemars(description = "Case-insensitive substring matched against table and column names")]
    pub pattern: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListTablesArgs {
    #[schemars(description = "Connection name as returned by list_connections")]
    pub connection: String,
    #[schemars(description = "Restrict to one schema")]
    pub schema: Option<String>,
    #[schemars(description = "Case-insensitive substring matched against table names")]
    pub name_like: Option<String>,
    #[schemars(description = "Maximum number of tables returned (default 100)")]
    pub limit: Option<u32>,
    #[schemars(description = "Number of tables to skip, for pagination")]
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DescribeTableArgs {
    #[schemars(description = "Connection name as returned by list_connections")]
    pub connection: String,
    #[schemars(description = "Table name, optionally schema-qualified (schema.table)")]
    pub table: String,
}

pub async fn search_schema(_state: &AppState, _args: SearchSchemaArgs) -> CallToolResult {
    ToolError::not_implemented("search_schema").into_call_result()
}

pub async fn list_tables(_state: &AppState, _args: ListTablesArgs) -> CallToolResult {
    ToolError::not_implemented("list_tables").into_call_result()
}

pub async fn describe_table(_state: &AppState, _args: DescribeTableArgs) -> CallToolResult {
    ToolError::not_implemented("describe_table").into_call_result()
}
