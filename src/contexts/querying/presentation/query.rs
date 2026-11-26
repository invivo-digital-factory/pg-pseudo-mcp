use rmcp::model::CallToolResult;
use rmcp::schemars;
use serde::Deserialize;

use super::error_mapping::tool_error;
use super::json_result::json_result;
use crate::contexts::querying::domain::repositories::ReadOnlyDatabase;
use crate::contexts::querying::domain::types::QueryRequest;
use crate::contexts::querying::use_cases::RunQuery;

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct QueryArgs {
    #[schemars(description = "Connection name as returned by list_connections")]
    pub connection: String,
    #[schemars(description = "One read-only SQL statement")]
    pub sql: String,
    #[schemars(description = "Maximum rows returned (default 50, maximum 500)")]
    pub max_rows: Option<u64>,
    #[schemars(
        description = "Maximum size of the returned rows in bytes (default 16384, maximum 65536)"
    )]
    pub max_bytes: Option<u64>,
    #[schemars(description = "Statement timeout in milliseconds (default 10000, maximum 60000)")]
    pub timeout_ms: Option<u64>,
    #[schemars(
        description = "Characters to skip at the start of every text cell before the 500-character cap; repeat a query with the offset given in a cell truncation to read the rest of a long value"
    )]
    pub cell_offset: Option<u64>,
    #[schemars(
        description = "Database of the same server to use instead of the connection's default database; names come from list_databases"
    )]
    pub database: Option<String>,
    #[schemars(
        description = "Run the request even if it is estimated heavy on a prod connection. Set it only after the developer explicitly approved the estimate returned by heavy_query_requires_confirmation; never on your own initiative"
    )]
    pub confirm_heavy: Option<bool>,
}

pub async fn query<D: ReadOnlyDatabase>(use_case: &RunQuery<D>, args: QueryArgs) -> CallToolResult {
    let request = QueryRequest {
        connection: args.connection,
        database: args.database,
        sql: args.sql,
        max_rows: args.max_rows,
        max_bytes: args.max_bytes,
        timeout_ms: args.timeout_ms,
        cell_offset: args.cell_offset,
        confirm_heavy: args.confirm_heavy.unwrap_or(false),
    };
    match use_case.execute(request).await {
        Ok(response) => json_result(&response),
        Err(error) => tool_error(error).into_call_result(),
    }
}
