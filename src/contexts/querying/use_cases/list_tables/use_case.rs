use std::sync::Arc;

use crate::contexts::querying::domain::constants::catalog::{DEFAULT_LIST_LIMIT, MAX_LIST_LIMIT};
use crate::contexts::querying::domain::errors::{DatabaseFailure, QueryingError, ScrubbedText};
use crate::contexts::querying::domain::repositories::{AnonymizationPorts, ReadOnlyDatabase};
use crate::contexts::querying::domain::services::catalog_queries::list_sql;
use crate::contexts::querying::domain::services::catalog_reading::table_entries;
use crate::contexts::querying::domain::types::{
    ExecutionLimits, ListResponse, ListTablesRequest, QueryLimits,
};

pub struct ListTables<D: ReadOnlyDatabase> {
    database: Arc<D>,
    anonymization: Result<AnonymizationPorts, String>,
}

impl<D: ReadOnlyDatabase> ListTables<D> {
    pub fn new(database: Arc<D>, anonymization: Result<AnonymizationPorts, String>) -> Self {
        Self {
            database,
            anonymization,
        }
    }

    pub async fn execute(&self, request: ListTablesRequest) -> Result<ListResponse, QueryingError> {
        let ports =
            self.anonymization
                .as_ref()
                .map_err(|reason| QueryingError::StoreUnavailable {
                    reason: reason.clone(),
                })?;
        let limits = QueryLimits::default();
        let limit = request
            .limit
            .unwrap_or(DEFAULT_LIST_LIMIT)
            .clamp(1, MAX_LIST_LIMIT);
        let offset = request.offset.unwrap_or(0);
        let output = self
            .database
            .read_catalog(
                &request.connection,
                &list_sql(
                    request.schema.as_deref(),
                    request.name_like.as_deref(),
                    offset,
                ),
                ExecutionLimits {
                    max_rows: limit as usize,
                    timeout_ms: limits.timeout_ms,
                },
            )
            .await
            .map_err(|failure| self.scrubbed(ports, failure, &limits))?;
        Ok(ListResponse {
            tables: table_entries(&output),
            next_offset: output
                .more_rows
                .then(|| u64::from(offset) + u64::from(limit)),
        })
    }

    fn scrubbed(
        &self,
        ports: &AnonymizationPorts,
        failure: DatabaseFailure,
        limits: &QueryLimits,
    ) -> QueryingError {
        let scrubbed = match &failure {
            DatabaseFailure::Db(message) => {
                ScrubbedText::Message(ports.anonymizer.message(message, &[]))
            }
            DatabaseFailure::Connect(text) | DatabaseFailure::Decode(text) => {
                ScrubbedText::Text(ports.anonymizer.free_text(text, &[]))
            }
            _ => ScrubbedText::Nothing,
        };
        QueryingError::from_failure(failure, limits.timeout_ms, scrubbed)
    }
}
