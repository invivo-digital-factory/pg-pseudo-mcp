use rmcp::model::CallToolResult;

use crate::contexts::querying::presentation;
use crate::state::AppState;

pub use crate::contexts::querying::presentation::QueryArgs;

pub async fn query(state: &AppState, args: QueryArgs) -> CallToolResult {
    presentation::query(&state.querying.run_query, args).await
}
