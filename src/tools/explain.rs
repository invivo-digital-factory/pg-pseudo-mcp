use rmcp::model::CallToolResult;
use rmcp::schemars;
use serde::{Deserialize, Serialize};

use crate::anon::apply::Anonymizer;
use crate::guard;
use crate::limits::QueryLimits;
use crate::pg::Cell;
use crate::state::AppState;
use crate::tools::errors::ToolError;
use crate::tools::query::database_error;
use crate::tools::result::{Warning, json_result};

const MAX_PLAN_LINES: usize = 500;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExplainArgs {
    #[schemars(description = "Connection name as returned by list_connections")]
    pub connection: String,
    #[schemars(description = "One read-only SQL statement to explain")]
    pub sql: String,
}

#[derive(Debug, Serialize)]
pub struct ExplainResponse {
    pub plan: Vec<String>,
    pub translated_literals: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<Warning>,
}

pub async fn explain(state: &AppState, args: ExplainArgs) -> CallToolResult {
    match run(state, args).await {
        Ok(response) => json_result(&response),
        Err(error) => error.into_call_result(),
    }
}

async fn run(state: &AppState, args: ExplainArgs) -> Result<ExplainResponse, ToolError> {
    let pseudonymizer = state.pseudonymizer()?;
    let guarded = guard::prepare(&args.sql, pseudonymizer.as_ref())?;
    let limits = QueryLimits::default();
    let session = state.session(&args.connection).await?;
    let anonymizer = Anonymizer::new(&pseudonymizer, &guarded.injected);
    let output = {
        let mut session = session.lock().await;
        session
            .run_read_only(&guarded.explain_sql(), MAX_PLAN_LINES, limits.timeout_ms)
            .await
            .map_err(|error| database_error(&anonymizer, error, &limits))?
    };
    let plan = output
        .rows
        .into_iter()
        .filter_map(|row| match row.into_iter().next() {
            Some(Cell::Text(line)) => Some(anonymizer.free_text(&line)),
            _ => None,
        })
        .collect();
    Ok(ExplainResponse {
        plan,
        translated_literals: guarded.translated_literals,
        warnings: guarded.warnings,
    })
}
