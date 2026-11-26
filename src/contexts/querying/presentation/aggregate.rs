use rmcp::model::CallToolResult;
use rmcp::schemars;
use serde::Deserialize;
use serde_json::Value;

use super::error_mapping::tool_error;
use super::json_result::json_result;
use crate::contexts::querying::domain::repositories::ReadOnlyDocumentStore;
use crate::contexts::querying::domain::types::AggregateRequest;
use crate::contexts::querying::use_cases::RunAggregate;

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct AggregateArgs {
    #[schemars(description = "Connection name as returned by list_connections (engine mongodb)")]
    pub connection: String,
    #[schemars(
        description = "Database to read; required when the connection declares no default database; names come from list_databases"
    )]
    pub database: Option<String>,
    #[schemars(
        description = "Collection or view the pipeline starts from, as returned by list_tables"
    )]
    pub collection: String,
    #[schemars(
        description = "Aggregation pipeline in Extended JSON: an array of read stages, for example [{\"$match\": {\"status\": \"paid\"}}, {\"$group\": {\"_id\": \"$status\", \"n\": {\"$sum\": 1}}}]"
    )]
    pub pipeline: Vec<Value>,
    #[serde(rename = "let")]
    #[schemars(
        rename = "let",
        description = "Variables of the pipeline in Extended JSON"
    )]
    pub let_vars: Option<Value>,
    #[schemars(description = "Maximum documents returned (default 50, maximum 500)")]
    pub max_rows: Option<u64>,
    #[schemars(
        description = "Maximum size of the returned documents in bytes (default 16384, maximum 65536)"
    )]
    pub max_bytes: Option<u64>,
    #[schemars(description = "Server time limit in milliseconds (default 10000, maximum 60000)")]
    pub timeout_ms: Option<u64>,
    #[schemars(
        description = "Characters to skip at the start of every text value before the 500-character cap; repeat a request with the offset given in a truncation to read the rest of a long value"
    )]
    pub cell_offset: Option<u64>,
    #[schemars(
        description = "Run the request even if it is estimated heavy on a prod connection. Set it only after the developer explicitly approved the estimate returned by heavy_query_requires_confirmation; never on your own initiative"
    )]
    pub confirm_heavy: Option<bool>,
}

pub async fn aggregate<S: ReadOnlyDocumentStore>(
    use_case: &RunAggregate<S>,
    args: AggregateArgs,
) -> CallToolResult {
    let request = AggregateRequest {
        connection: args.connection,
        database: args.database,
        collection: args.collection,
        pipeline: args.pipeline,
        let_vars: args.let_vars,
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
