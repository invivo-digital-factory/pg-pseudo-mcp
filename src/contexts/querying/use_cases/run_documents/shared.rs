use std::collections::HashMap;
use std::time::Instant;

use crate::contexts::querying::domain::constants::heavy_query::CHECK_SKIPPED;
use crate::contexts::querying::domain::constants::mongo_policy::{
    ARRAY_HINT, BYTES_HINT, DECODED_BUDGET_FACTOR, FIELDS_HINT, KEYSET_HINT,
    MAX_RETURNED_ARRAY_ITEMS,
};
use crate::contexts::querying::domain::errors::QueryingError;
use crate::contexts::querying::domain::repositories::{DocumentPorts, ReadOnlyDocumentStore};
use crate::contexts::querying::domain::services::document_bounds::{
    cap_arrays, cut_texts, fit_documents, prune_decoded, shown_path,
};
use crate::contexts::querying::domain::services::document_clearances::{
    DocumentCommand, document_clearances, document_hints, observed_paths,
};
use crate::contexts::querying::domain::services::document_requests::scrubbed;
use crate::contexts::querying::domain::services::ejson_render::render;
use crate::contexts::querying::domain::services::heavy_query::{
    collection_scans, mongo_verdict, needs_count,
};
use crate::contexts::querying::domain::services::mongo_lineage::document_lineage;
use crate::contexts::querying::domain::types::{
    ClampWarning, ColumnPolicy, DocumentOutput, DocumentsResponse, ExecutionLimits, GuardedMongo,
    HeavyQueryEstimate, HeavyQueryThresholds, MongoCommand, QueryLimits, TruncatedBy, Truncation,
    Warning,
};

pub struct Execution<'a, S: ReadOnlyDocumentStore> {
    pub store: &'a S,
    pub ports: &'a DocumentPorts,
    pub policy: ColumnPolicy,
    pub connection: &'a str,
    pub database: &'a str,
    pub command: DocumentCommand,
    pub limits: QueryLimits,
    pub clamps: Vec<ClampWarning>,
    pub cell_offset: Option<u64>,
    pub confirm_heavy: bool,
    pub started: Instant,
}

enum HeavyCheck {
    Light,
    Heavy(HeavyQueryEstimate),
    Skipped,
}

impl<S: ReadOnlyDocumentStore> Execution<'_, S> {
    pub async fn run(self, mut guarded: GuardedMongo) -> Result<DocumentsResponse, QueryingError> {
        let execution = ExecutionLimits {
            max_rows: self.limits.max_rows,
            timeout_ms: self.limits.timeout_ms,
        };
        if !self.confirm_heavy
            && let Some(thresholds) = self.store.heavy_query_thresholds(self.connection).await
        {
            match self.heavy_check(&guarded, execution, thresholds).await {
                HeavyCheck::Heavy(estimate) => return Err(QueryingError::HeavyQuery(estimate)),
                HeavyCheck::Skipped => guarded.warnings.push(Warning::new(
                    CHECK_SKIPPED,
                    "the heavy-query estimate could not be made for this request on a prod connection; it ran without it",
                )),
                HeavyCheck::Light => {}
            }
        }
        let outcome = match &guarded.command {
            MongoCommand::Find(find) => {
                self.store
                    .find(
                        self.connection,
                        self.database,
                        &guarded.collection,
                        find,
                        execution,
                    )
                    .await
            }
            MongoCommand::Aggregate(aggregate) => {
                self.store
                    .aggregate(
                        self.connection,
                        self.database,
                        &guarded.collection,
                        aggregate,
                        execution,
                    )
                    .await
            }
        };
        let output = outcome.map_err(|failure| {
            scrubbed(
                self.ports,
                failure,
                self.limits.timeout_ms,
                &guarded.injected,
            )
        })?;
        Ok(self.respond(guarded, output))
    }

    async fn heavy_check(
        &self,
        guarded: &GuardedMongo,
        execution: ExecutionLimits,
        thresholds: HeavyQueryThresholds,
    ) -> HeavyCheck {
        let explained = match &guarded.command {
            MongoCommand::Find(find) => {
                self.store
                    .explain_find(
                        self.connection,
                        self.database,
                        &guarded.collection,
                        find,
                        execution,
                    )
                    .await
            }
            MongoCommand::Aggregate(aggregate) => {
                self.store
                    .explain_aggregate(
                        self.connection,
                        self.database,
                        &guarded.collection,
                        aggregate,
                        execution,
                    )
                    .await
            }
        };
        let Ok(explained) = explained else {
            return HeavyCheck::Skipped;
        };
        let scans = collection_scans(&explained);
        let mut counts = HashMap::new();
        for scan in scans.iter().filter(|scan| needs_count(scan)) {
            if counts.contains_key(&scan.collection) {
                continue;
            }
            if let Ok(Some(count)) = self
                .store
                .estimated_count(self.connection, self.database, &scan.collection)
                .await
            {
                counts.insert(scan.collection.clone(), count);
            }
        }
        match mongo_verdict(&scans, &counts, thresholds) {
            Some(estimate) => HeavyCheck::Heavy(estimate),
            None => HeavyCheck::Light,
        }
    }

    fn respond(self, guarded: GuardedMongo, output: DocumentOutput) -> DocumentsResponse {
        let cell_offset = usize::try_from(self.cell_offset.unwrap_or(0)).unwrap_or(usize::MAX);
        let mut documents = output.documents;
        let array_cuts = cap_arrays(&mut documents, MAX_RETURNED_ARRAY_ITEMS);
        let (documents, pruned) = prune_decoded(
            documents,
            self.limits.max_bytes.saturating_mul(DECODED_BUDGET_FACTOR),
        );
        let lineage = document_lineage(&guarded, self.database);
        let clearances = document_clearances(
            &observed_paths(&documents),
            &lineage,
            &self.policy,
            (self.database, &guarded.collection),
            self.command,
        );
        let mut anonymized =
            self.ports
                .anonymizer
                .documents(documents, &clearances, &guarded.injected);
        let column_policy_hints = document_hints(&anonymized.pseudonymized, &lineage, &self.policy);
        let mut truncated = Vec::new();
        if let Some(position) = array_cuts
            .iter()
            .find(|position| position.document < anonymized.documents.len())
        {
            truncated.push(Truncation {
                by: TruncatedBy::Array,
                limit: MAX_RETURNED_ARRAY_ITEMS as u64,
                column: None,
                field: Some(shown_path(&anonymized.documents, position)),
                hint: ARRAY_HINT.into(),
            });
        }
        if let Some(position) = cut_texts(
            &mut anonymized.documents,
            cell_offset,
            self.limits.max_cell_chars,
        ) {
            truncated.push(Truncation {
                by: TruncatedBy::Cell,
                limit: self.limits.max_cell_chars as u64,
                column: None,
                field: Some(shown_path(&anonymized.documents, &position)),
                hint: format!(
                    "long values are cut; repeat the same request with cell_offset={} to read the rest",
                    cell_offset.saturating_add(self.limits.max_cell_chars)
                ),
            });
        }
        let rendered = anonymized.documents.into_iter().map(render).collect();
        let (documents, fitted) = fit_documents(rendered, self.limits.max_bytes);
        if pruned || fitted {
            truncated.push(Truncation {
                by: TruncatedBy::Bytes,
                limit: self.limits.max_bytes as u64,
                column: None,
                field: None,
                hint: format!("{BYTES_HINT}; {KEYSET_HINT}"),
            });
        } else if output.more_documents {
            truncated.push(Truncation {
                by: TruncatedBy::Rows,
                limit: self.limits.max_rows as u64,
                column: None,
                field: None,
                hint: KEYSET_HINT.into(),
            });
        }
        if anonymized.omitted_paths > 0 {
            truncated.push(Truncation {
                by: TruncatedBy::Fields,
                limit: anonymized.pseudonymized.len() as u64,
                column: None,
                field: None,
                hint: FIELDS_HINT.into(),
            });
        }
        let mut warnings = guarded.warnings.clone();
        warnings.extend(self.clamps.iter().chain(&guarded.clamps).map(|clamp| {
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
                    "every text value starts at character {cell_offset}; shorter values are empty"
                ),
            ));
        }
        DocumentsResponse {
            document_count: documents.len(),
            documents,
            pseudonymized: anonymized.pseudonymized,
            column_policy_hints,
            translated_literals: guarded.translated_literals,
            truncated,
            warnings,
            elapsed_ms: self.started.elapsed().as_millis(),
        }
    }
}
