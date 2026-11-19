use rmcp::model::CallToolResult;

use crate::contexts::querying::presentation;
use crate::state::AppState;

pub use crate::contexts::querying::presentation::ExplainArgs;

pub async fn explain(state: &AppState, args: ExplainArgs) -> CallToolResult {
    presentation::explain(&state.querying.explain_query, args).await
}
