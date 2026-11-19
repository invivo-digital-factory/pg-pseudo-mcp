use rmcp::model::CallToolResult;

use crate::contexts::querying::presentation;
use crate::state::AppState;

pub use crate::contexts::querying::presentation::{
    DescribeTableArgs, ListTablesArgs, SearchSchemaArgs,
};

pub async fn search_schema(state: &AppState, args: SearchSchemaArgs) -> CallToolResult {
    presentation::search_schema(&state.querying.search_schema, args).await
}

pub async fn list_tables(state: &AppState, args: ListTablesArgs) -> CallToolResult {
    presentation::list_tables(&state.querying.list_tables, args).await
}

pub async fn describe_table(state: &AppState, args: DescribeTableArgs) -> CallToolResult {
    presentation::describe_table(&state.querying.describe_table, args).await
}
