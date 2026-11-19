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
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
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
    let anonymizer = Anonymizer::new(&pseudonymizer, &guarded.injected);
    let output = {
        let mut session = state.session(&args.connection).await?;
        session
            .run_read_only(&guarded.explain_sql(), MAX_PLAN_LINES, limits.timeout_ms)
            .await
            .map_err(|error| database_error(&anonymizer, error, &limits))?
    };
    let mut warnings = guarded.warnings;
    let mut plan = Vec::new();
    for cell in output
        .rows
        .into_iter()
        .filter_map(|row| row.into_iter().next())
    {
        match cell {
            Cell::Text(text) => plan.extend(text.lines().map(|line| anonymizer.free_text(line))),
            Cell::Json(value) => {
                let text = serde_json::to_string_pretty(&value).unwrap_or_default();
                plan.extend(text.lines().map(|line| anonymizer.free_text(line)));
            }
            Cell::Null => {}
            other => {
                let shown = match other {
                    Cell::Opaque { type_name, .. } => type_name,
                    _ => "an unsupported type".to_string(),
                };
                warnings.push(Warning::new(
                    "plan_format_unsupported",
                    format!("the plan came back as {shown}, which is not shown; use FORMAT TEXT, JSON or YAML"),
                ));
                break;
            }
        }
    }
    let truncated = output.more_rows || plan.len() > MAX_PLAN_LINES;
    plan.truncate(MAX_PLAN_LINES);
    Ok(ExplainResponse {
        plan,
        truncated,
        translated_literals: guarded.translated_literals,
        warnings,
    })
}
