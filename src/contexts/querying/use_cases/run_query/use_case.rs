use std::sync::Arc;
use std::time::Instant;

use crate::contexts::querying::domain::constants::clear_lineage::{
    MAX_LINEAGE_RELATIONS, MAX_RELATION_COLUMNS, RELATION_COLUMNS_TIMEOUT_MS,
};
use crate::contexts::querying::domain::errors::{DatabaseFailure, QueryingError, ScrubbedText};
use crate::contexts::querying::domain::repositories::{
    AnonymizationPorts, ColumnPolicySource, ReadOnlyDatabase,
};
use crate::contexts::querying::domain::services::catalog_queries::relation_columns_sql;
use crate::contexts::querying::domain::services::catalog_reading::relation_columns;
use crate::contexts::querying::domain::services::clear_lineage::{
    catalog_relations, clearances, is_clear_source, is_clear_source_by_policy,
};
use crate::contexts::querying::domain::services::column_policy::{
    combine_lineage, policy_clearances, result_hints,
};
use crate::contexts::querying::domain::services::heavy_query::{
    explain_for_cost, pg_verdict, relation_rows, relation_rows_sql, summarize_pg_plan,
};
use crate::contexts::querying::domain::services::result_bounds::bound_rows;
use crate::contexts::querying::domain::services::statement_guard::inspect;
use crate::contexts::querying::domain::types::{
    Cell, Clearance, ColumnMeta, ColumnOut, ColumnPolicy, ExecutionLimits, Guarded, NoticeOut,
    PersonalNames, QueryLimits, QueryRequest, QueryResponse, RelationCatalog, Translations,
    TruncatedBy, Truncation, Warning,
};

pub struct RunQuery<D: ReadOnlyDatabase> {
    database: Arc<D>,
    anonymization: Result<AnonymizationPorts, String>,
    policy: Option<Arc<dyn ColumnPolicySource>>,
}

impl<D: ReadOnlyDatabase> RunQuery<D> {
    pub fn new(database: Arc<D>, anonymization: Result<AnonymizationPorts, String>) -> Self {
        Self {
            database,
            anonymization,
            policy: None,
        }
    }

    pub fn with_column_policy(mut self, policy: Arc<dyn ColumnPolicySource>) -> Self {
        self.policy = Some(policy);
        self
    }

    pub async fn execute(&self, request: QueryRequest) -> Result<QueryResponse, QueryingError> {
        let started = Instant::now();
        let ports =
            self.anonymization
                .as_ref()
                .map_err(|reason| QueryingError::StoreUnavailable {
                    reason: reason.clone(),
                })?;
        let policy = self.column_policy();
        let guarded = self.guard(ports, &policy, &request.sql)?;
        let (limits, clamps) =
            QueryLimits::from_request(request.max_rows, request.max_bytes, request.timeout_ms);
        let cell_offset = usize::try_from(request.cell_offset.unwrap_or(0)).unwrap_or(usize::MAX);
        if !request.confirm_heavy && !guarded.is_explain {
            self.refuse_heavy(ports, &request, &guarded, &limits)
                .await?;
        }
        let output = self
            .database
            .run(
                &request.connection,
                request.database.as_deref(),
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
        let enum_labels = self
            .database
            .enum_labels(&request.connection, request.database.as_deref())
            .await
            .unwrap_or_default();
        let clearances = self
            .clearances(ports, &policy, &request, &guarded, &output.columns)
            .await;
        let anonymized = ports.anonymizer.rows(
            &output.columns,
            output.rows,
            guarded.derived_lineage.as_deref(),
            &guarded.injected,
            &enum_labels,
            &clearances,
        );
        let column_policy_hints = result_hints(&output.columns, &anonymized.per_column, &policy);
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
                field: None,
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
            column_policy_hints,
            translated_literals: guarded.translated_literals,
            truncated,
            warnings,
            notices,
            elapsed_ms: started.elapsed().as_millis(),
        })
    }

    async fn refuse_heavy(
        &self,
        ports: &AnonymizationPorts,
        request: &QueryRequest,
        guarded: &Guarded,
        limits: &QueryLimits,
    ) -> Result<(), QueryingError> {
        let Some(thresholds) = self
            .database
            .heavy_query_thresholds(&request.connection)
            .await
        else {
            return Ok(());
        };
        let execution = ExecutionLimits {
            max_rows: 1,
            timeout_ms: limits.timeout_ms,
        };
        let plan = self
            .database
            .explain(
                &request.connection,
                request.database.as_deref(),
                &explain_for_cost(&guarded.sql),
                execution,
            )
            .await
            .map_err(|failure| self.scrubbed(ports, failure, limits, &guarded.injected))?;
        let Some(summary) =
            plan.rows
                .first()
                .and_then(|row| row.first())
                .and_then(|cell| match cell {
                    Cell::Json(value) => summarize_pg_plan(value),
                    _ => None,
                })
        else {
            return Ok(());
        };
        let rows = match relation_rows_sql(&summary) {
            Some(sql) => self
                .database
                .read_catalog(
                    &request.connection,
                    request.database.as_deref(),
                    &sql,
                    ExecutionLimits {
                        max_rows: summary.scans.len(),
                        timeout_ms: limits.timeout_ms,
                    },
                )
                .await
                .map(|output| relation_rows(&output))
                .unwrap_or_default(),
            None => Default::default(),
        };
        match pg_verdict(&summary, &rows, thresholds) {
            Some(estimate) => Err(QueryingError::HeavyQuery(estimate)),
            None => Ok(()),
        }
    }

    fn column_policy(&self) -> ColumnPolicy {
        self.policy
            .as_ref()
            .map(|source| source.load().policy)
            .unwrap_or_default()
    }

    async fn clearances(
        &self,
        ports: &AnonymizationPorts,
        policy: &ColumnPolicy,
        request: &QueryRequest,
        guarded: &Guarded,
        columns: &[ColumnMeta],
    ) -> Vec<Clearance> {
        let lineage = self
            .lineage_clearances(ports, policy, request, guarded, columns)
            .await;
        policy_clearances(columns, policy, lineage)
    }

    async fn lineage_clearances(
        &self,
        ports: &AnonymizationPorts,
        policy: &ColumnPolicy,
        request: &QueryRequest,
        guarded: &Guarded,
        columns: &[ColumnMeta],
    ) -> Vec<Clearance> {
        let unproven = vec![Clearance::Unproven; columns.len()];
        if columns.iter().all(|column| column.source.is_some()) {
            return unproven;
        }
        let relations = catalog_relations(&guarded.statement);
        if relations.len() > MAX_LINEAGE_RELATIONS {
            return unproven;
        }
        let mut catalog = RelationCatalog::default();
        for relation in relations {
            let read = self
                .database
                .read_catalog_cached(
                    &request.connection,
                    request.database.as_deref(),
                    &relation_columns_sql(&relation),
                    ExecutionLimits {
                        max_rows: MAX_RELATION_COLUMNS,
                        timeout_ms: RELATION_COLUMNS_TIMEOUT_MS,
                    },
                )
                .await;
            if let Some(columns) = read.ok().as_ref().and_then(relation_columns) {
                catalog.insert(&relation, columns);
            }
        }
        let dictionary = ports.dictionary.as_ref();
        let proven = clearances(
            &guarded.statement,
            &catalog,
            &|column| is_clear_source(dictionary, policy, column),
            columns.len(),
        );
        if !policy.has_clear() {
            return proven;
        }
        let with_policy = clearances(
            &guarded.statement,
            &catalog,
            &|column| is_clear_source_by_policy(dictionary, policy, column),
            columns.len(),
        );
        combine_lineage(proven, with_policy)
    }

    fn guard(
        &self,
        ports: &AnonymizationPorts,
        policy: &ColumnPolicy,
        sql: &str,
    ) -> Result<Guarded, QueryingError> {
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
            if ports.dictionary.dictionary_kind(&name).is_some() || policy.is_personal_name(&name) {
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
            DatabaseFailure::Connect(text)
            | DatabaseFailure::Decode(text)
            | DatabaseFailure::ConnectFailed { reason: text, .. } => {
                ScrubbedText::Text(ports.anonymizer.free_text(text, injected))
            }
            _ => ScrubbedText::Nothing,
        };
        QueryingError::from_failure(failure, limits.timeout_ms, scrubbed)
    }
}
