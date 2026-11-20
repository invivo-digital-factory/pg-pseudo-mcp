use std::sync::Arc;
use std::time::Instant;

use crate::contexts::querying::domain::errors::{DatabaseFailure, QueryingError, ScrubbedText};
use crate::contexts::querying::domain::repositories::{AnonymizationPorts, ReadOnlyDatabase};
use crate::contexts::querying::domain::services::result_bounds::bound_rows;
use crate::contexts::querying::domain::services::statement_guard::inspect;
use crate::contexts::querying::domain::types::{
    ColumnOut, ExecutionLimits, Guarded, NoticeOut, PersonalNames, QueryLimits, QueryRequest,
    QueryResponse, Translations, TruncatedBy, Truncation, Warning,
};

pub struct RunQuery<D: ReadOnlyDatabase> {
    database: Arc<D>,
    anonymization: Result<AnonymizationPorts, String>,
}

impl<D: ReadOnlyDatabase> RunQuery<D> {
    pub fn new(database: Arc<D>, anonymization: Result<AnonymizationPorts, String>) -> Self {
        Self {
            database,
            anonymization,
        }
    }

    pub async fn execute(&self, request: QueryRequest) -> Result<QueryResponse, QueryingError> {
        let started = Instant::now();
        let ports =
            self.anonymization
                .as_ref()
                .map_err(|reason| QueryingError::StoreUnavailable {
                    reason: reason.clone(),
                })?;
        let guarded = self.guard(ports, &request.sql)?;
        let (limits, clamps) =
            QueryLimits::from_request(request.max_rows, request.max_bytes, request.timeout_ms);
        let cell_offset = usize::try_from(request.cell_offset.unwrap_or(0)).unwrap_or(usize::MAX);
        let output = self
            .database
            .run(
                &request.connection,
                &guarded.sql,
                ExecutionLimits {
                    max_rows: limits.max_rows,
                    timeout_ms: limits.timeout_ms,
                },
            )
            .await
            .map_err(|failure| self.scrubbed(ports, failure, &limits, &guarded.injected))?;
        let mut warnings = guarded.warnings.clone();
        warnings.extend(clamps.into_iter().map(|clamp| {
            Warning::new(
                clamp.code,
                format!(
                    "requested {} was capped to {}",
                    clamp.requested, clamp.applied
                ),
            )
        }));
        if cell_offset > 0 {
            warnings.push(Warning::new(
                "cell_offset_applied",
                format!(
                    "every text cell starts at character {cell_offset}; shorter values are empty"
                ),
            ));
        }
        let anonymized = ports.anonymizer.rows(
            &output.columns,
            output.rows,
            guarded.derived_lineage.as_deref(),
            &guarded.injected,
        );
        let columns: Vec<ColumnOut> = output
            .columns
            .iter()
            .zip(&anonymized.per_column)
            .map(|(column, flag)| ColumnOut {
                name: column.name.clone(),
                type_name: column.type_name.clone(),
                pseudonymized: flag.clone(),
            })
            .collect();
        let mut truncated = Vec::new();
        let rows = bound_rows(
            anonymized.rows,
            &output.columns,
            &limits,
            cell_offset,
            &mut truncated,
        );
        if output.more_rows && !truncated.iter().any(|item| item.by == TruncatedBy::Bytes) {
            truncated.push(Truncation {
                by: TruncatedBy::Rows,
                limit: limits.max_rows as u64,
                column: None,
                hint: "narrow the filter or page with a keyset condition (WHERE id > last_id ORDER BY id) instead of OFFSET".into(),
            });
        }
        let notices = output
            .notices
            .iter()
            .map(|notice| {
                let scrubbed = ports.anonymizer.message(notice, &guarded.injected);
                NoticeOut {
                    severity: scrubbed.severity,
                    message: scrubbed.message,
                    detail: scrubbed.detail,
                }
            })
            .collect();
        Ok(QueryResponse {
            columns,
            row_count: rows.len(),
            rows,
            pseudonymized: anonymized.pseudonymized,
            translated_literals: guarded.translated_literals,
            truncated,
            warnings,
            notices,
            elapsed_ms: started.elapsed().as_millis(),
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
        let mut personal = PersonalNames::default();
        for name in rewritten.lineage_names() {
            if ports.dictionary.dictionary_kind(&name).is_some() {
                personal.insert(&name);
            }
        }
        Ok(rewritten.finish(&personal))
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
