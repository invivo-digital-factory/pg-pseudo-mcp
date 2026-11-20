use std::sync::Arc;

use crate::contexts::querying::domain::constants::catalog::MAX_PLAN_LINES;
use crate::contexts::querying::domain::errors::{DatabaseFailure, QueryingError, ScrubbedText};
use crate::contexts::querying::domain::repositories::{AnonymizationPorts, ReadOnlyDatabase};
use crate::contexts::querying::domain::services::statement_guard::inspect;
use crate::contexts::querying::domain::types::{
    Cell, ExecutionLimits, ExplainRequest, ExplainResponse, Guarded, QueryLimits, Translations,
    Warning,
};

pub struct ExplainQuery<D: ReadOnlyDatabase> {
    database: Arc<D>,
    anonymization: Result<AnonymizationPorts, String>,
}

impl<D: ReadOnlyDatabase> ExplainQuery<D> {
    pub fn new(database: Arc<D>, anonymization: Result<AnonymizationPorts, String>) -> Self {
        Self {
            database,
            anonymization,
        }
    }

    pub async fn execute(&self, request: ExplainRequest) -> Result<ExplainResponse, QueryingError> {
        let ports =
            self.anonymization
                .as_ref()
                .map_err(|reason| QueryingError::StoreUnavailable {
                    reason: reason.clone(),
                })?;
        let guarded = self.guard(ports, &request.sql)?;
        let limits = QueryLimits::default();
        let output = self
            .database
            .explain(
                &request.connection,
                &guarded.explain_sql(),
                ExecutionLimits {
                    max_rows: MAX_PLAN_LINES,
                    timeout_ms: limits.timeout_ms,
                },
            )
            .await
            .map_err(|failure| self.scrubbed(ports, failure, &limits, &guarded.injected))?;
        let mut warnings = guarded.warnings;
        let mut plan = Vec::new();
        for cell in output
            .rows
            .into_iter()
            .filter_map(|row| row.into_iter().next())
        {
            match cell {
                Cell::Text(text) => plan.extend(
                    text.lines()
                        .map(|line| ports.anonymizer.free_text(line, &guarded.injected)),
                ),
                Cell::Json(value) => {
                    let scrubbed = ports.anonymizer.plan(value, &guarded.injected);
                    let text = serde_json::to_string_pretty(&scrubbed).unwrap_or_default();
                    plan.extend(text.lines().map(str::to_string));
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

    fn guard(&self, ports: &AnonymizationPorts, sql: &str) -> Result<Guarded, QueryingError> {
        let inspected = inspect(sql).map_err(QueryingError::Refused)?;
        let mut translations = Translations::default();
        for literal in inspected.literals() {
            translations.record(literal, ports.translator.lookup(literal));
        }
        let rewritten = inspected
            .rewrite(&translations)
            .map_err(QueryingError::Refused)?;
        Ok(rewritten.finish_without_lineage())
    }

    fn scrubbed(
        &self,
        ports: &AnonymizationPorts,
        failure: DatabaseFailure,
        limits: &QueryLimits,
        injected: &[(String, String)],
    ) -> QueryingError {
        let scrubbed = match &failure {
            DatabaseFailure::Db(message) => {
                ScrubbedText::Message(ports.anonymizer.message(message, injected))
            }
            DatabaseFailure::Connect(text) | DatabaseFailure::Decode(text) => {
                ScrubbedText::Text(ports.anonymizer.free_text(text, injected))
            }
            _ => ScrubbedText::Nothing,
        };
        QueryingError::from_failure(failure, limits.timeout_ms, scrubbed)
    }
}
