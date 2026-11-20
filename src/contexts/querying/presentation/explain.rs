use rmcp::model::CallToolResult;
use rmcp::schemars;
use serde::Deserialize;

use super::error_mapping::tool_error;
use super::json_result::json_result;
use crate::contexts::querying::domain::repositories::ReadOnlyDatabase;
use crate::contexts::querying::domain::types::ExplainRequest;
use crate::contexts::querying::use_cases::ExplainQuery;

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct ExplainArgs {
    #[schemars(description = "Connection name as returned by list_connections")]
    pub connection: String,
    #[schemars(description = "One read-only SQL statement to explain")]
    pub sql: String,
    #[schemars(
        description = "Database of the same server to use instead of the connection's default database; names come from list_databases"
    )]
    pub database: Option<String>,
}

pub async fn explain<D: ReadOnlyDatabase>(
    use_case: &ExplainQuery<D>,
    args: ExplainArgs,
) -> CallToolResult {
    let request = ExplainRequest {
        connection: args.connection,
        database: args.database,
        sql: args.sql,
    };
    match use_case.execute(request).await {
        Ok(response) => json_result(&response),
        Err(error) => tool_error(error).into_call_result(),
    }
}
