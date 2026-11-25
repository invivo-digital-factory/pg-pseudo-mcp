use rmcp::model::CallToolResult;
use rmcp::schemars;
use serde::Deserialize;

use super::error_mapping::tool_error;
use super::json_result::json_result;
use crate::contexts::querying::domain::repositories::{ReadOnlyDatabase, ReadOnlyDocumentStore};
use crate::contexts::querying::domain::types::ListTablesRequest;
use crate::contexts::querying::use_cases::{ListCollections, ListTables};

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct ListTablesArgs {
    #[schemars(description = "Connection name as returned by list_connections")]
    pub connection: String,
    #[schemars(description = "Restrict to one schema")]
    pub schema: Option<String>,
    #[schemars(description = "Case-insensitive substring matched against table names")]
    pub name_like: Option<String>,
    #[schemars(description = "Maximum number of tables returned (default 100, maximum 500)")]
    pub limit: Option<u32>,
    #[schemars(description = "Number of tables to skip, for pagination")]
    pub offset: Option<u32>,
    #[schemars(
        description = "Database of the same server to use instead of the connection's default database; names come from list_databases"
    )]
    pub database: Option<String>,
}

pub async fn list_tables<D: ReadOnlyDatabase>(
    use_case: &ListTables<D>,
    args: ListTablesArgs,
) -> CallToolResult {
    let request = ListTablesRequest {
        connection: args.connection,
        database: args.database,
        schema: args.schema,
        name_like: args.name_like,
        limit: args.limit,
        offset: args.offset,
    };
    match use_case.execute(request).await {
        Ok(response) => json_result(&response),
        Err(error) => tool_error(error).into_call_result(),
    }
}

pub async fn list_collections<S: ReadOnlyDocumentStore>(
    use_case: &ListCollections<S>,
    args: ListTablesArgs,
) -> CallToolResult {
    let request = ListTablesRequest {
        connection: args.connection,
        database: args.database,
        schema: args.schema,
        name_like: args.name_like,
        limit: args.limit,
        offset: args.offset,
    };
    match use_case.execute(request).await {
        Ok(response) => json_result(&response),
        Err(error) => tool_error(error).into_call_result(),
    }
}
