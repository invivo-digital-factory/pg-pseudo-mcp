use rmcp::model::CallToolResult;
use rmcp::schemars;
use serde::Deserialize;

use super::error_mapping::tool_error;
use super::json_result::json_result;
use crate::contexts::querying::domain::repositories::ReadOnlyDatabase;
use crate::contexts::querying::domain::types::ListDatabasesRequest;
use crate::contexts::querying::use_cases::ListDatabases;

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct ListDatabasesArgs {
    #[schemars(description = "Connection name as returned by list_connections")]
    pub connection: String,
}

pub async fn list_databases<D: ReadOnlyDatabase>(
    use_case: &ListDatabases<D>,
    args: ListDatabasesArgs,
) -> CallToolResult {
    let request = ListDatabasesRequest {
        connection: args.connection,
    };
    match use_case.execute(request).await {
        Ok(response) => json_result(&response),
        Err(error) => tool_error(error).into_call_result(),
    }
}
