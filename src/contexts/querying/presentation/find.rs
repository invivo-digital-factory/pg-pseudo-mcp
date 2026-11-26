use rmcp::model::CallToolResult;
use rmcp::schemars;
use serde::Deserialize;
use serde_json::Value;

use super::error_mapping::tool_error;
use super::json_result::json_result;
use crate::contexts::querying::domain::repositories::ReadOnlyDocumentStore;
use crate::contexts::querying::domain::types::FindRequest;
use crate::contexts::querying::use_cases::RunFind;

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct FindArgs {
    #[schemars(description = "Connection name as returned by list_connections (engine mongodb)")]
    pub connection: String,
    #[schemars(
        description = "Database to read; required when the connection declares no default database; names come from list_databases"
    )]
    pub database: Option<String>,
    #[schemars(description = "Collection or view name, as returned by list_tables")]
    pub collection: String,
    #[schemars(
        description = "Query filter in Extended JSON, for example {\"status\": \"paid\", \"_id\": {\"$oid\": \"...\"}}"
    )]
    pub filter: Option<Value>,
    #[schemars(description = "Projection in Extended JSON, for example {\"status\": 1}")]
    pub projection: Option<Value>,
    #[schemars(description = "Sort in Extended JSON, for example {\"_id\": 1}")]
    pub sort: Option<Value>,
    #[schemars(description = "Documents to skip; prefer a keyset filter on _id")]
    pub skip: Option<u64>,
    #[schemars(description = "Maximum documents to read, capped at max_rows")]
    pub limit: Option<u64>,
    #[schemars(description = "Index name or key pattern to use")]
    pub hint: Option<Value>,
    #[schemars(description = "Collation in Extended JSON")]
    pub collation: Option<Value>,
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

pub async fn find<S: ReadOnlyDocumentStore>(
    use_case: &RunFind<S>,
    args: FindArgs,
) -> CallToolResult {
    let request = FindRequest {
        connection: args.connection,
        database: args.database,
        collection: args.collection,
        filter: args.filter,
        projection: args.projection,
        sort: args.sort,
        skip: args.skip,
        limit: args.limit,
        hint: args.hint,
        collation: args.collation,
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
