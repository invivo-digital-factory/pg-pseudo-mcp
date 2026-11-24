use rmcp::model::CallToolResult;
use rmcp::schemars;
use serde::Deserialize;

use super::error_mapping::tool_error;
use super::json_result::json_result;
use crate::contexts::querying::domain::repositories::{ReadOnlyDatabase, ReadOnlyDocumentStore};
use crate::contexts::querying::domain::types::SearchSchemaRequest;
use crate::contexts::querying::use_cases::{SearchCollections, SearchSchema};

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct SearchSchemaArgs {
    #[schemars(description = "Connection name as returned by list_connections")]
    pub connection: String,
    #[schemars(description = "Case-insensitive substring matched against table and column names")]
    pub pattern: String,
    #[schemars(
        description = "Database of the same server to use instead of the connection's default database; names come from list_databases"
    )]
    pub database: Option<String>,
}

pub async fn search_schema<D: ReadOnlyDatabase>(
    use_case: &SearchSchema<D>,
    args: SearchSchemaArgs,
) -> CallToolResult {
    let request = SearchSchemaRequest {
        connection: args.connection,
        database: args.database,
        pattern: args.pattern,
    };
    match use_case.execute(request).await {
        Ok(response) => json_result(&response),
        Err(error) => tool_error(error).into_call_result(),
    }
}

pub async fn search_collections<S: ReadOnlyDocumentStore>(
    use_case: &SearchCollections<S>,
    args: SearchSchemaArgs,
) -> CallToolResult {
    let request = SearchSchemaRequest {
        connection: args.connection,
        database: args.database,
        pattern: args.pattern,
    };
    match use_case.execute(request).await {
        Ok(response) => json_result(&response),
        Err(error) => tool_error(error).into_call_result(),
    }
}
