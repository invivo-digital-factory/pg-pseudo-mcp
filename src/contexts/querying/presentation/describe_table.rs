use rmcp::model::CallToolResult;
use rmcp::schemars;
use serde::Deserialize;

use super::error_mapping::tool_error;
use super::json_result::json_result;
use crate::contexts::querying::domain::repositories::ReadOnlyDatabase;
use crate::contexts::querying::domain::types::DescribeTableRequest;
use crate::contexts::querying::use_cases::DescribeTable;

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct DescribeTableArgs {
    #[schemars(description = "Connection name as returned by list_connections")]
    pub connection: String,
    #[schemars(description = "Table name, optionally schema-qualified (schema.table)")]
    pub table: String,
    #[schemars(
        description = "Database of the same server to use instead of the connection's default database; names come from list_databases"
    )]
    pub database: Option<String>,
}

pub async fn describe_table<D: ReadOnlyDatabase>(
    use_case: &DescribeTable<D>,
    args: DescribeTableArgs,
) -> CallToolResult {
    let request = DescribeTableRequest {
        connection: args.connection,
        database: args.database,
        table: args.table,
    };
    match use_case.execute(request).await {
        Ok(response) => json_result(&response),
        Err(error) => tool_error(error).into_call_result(),
    }
}
