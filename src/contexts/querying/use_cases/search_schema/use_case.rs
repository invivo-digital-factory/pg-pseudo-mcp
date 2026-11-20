use std::sync::Arc;

use crate::contexts::querying::domain::constants::catalog::SEARCH_LIMIT;
use crate::contexts::querying::domain::errors::{DatabaseFailure, QueryingError, ScrubbedText};
use crate::contexts::querying::domain::repositories::{AnonymizationPorts, ReadOnlyDatabase};
use crate::contexts::querying::domain::services::catalog_queries::search_sql;
use crate::contexts::querying::domain::services::catalog_reading::schema_matches;
use crate::contexts::querying::domain::types::{
    ExecutionLimits, QueryLimits, SearchResponse, SearchSchemaRequest,
};

pub struct SearchSchema<D: ReadOnlyDatabase> {
    database: Arc<D>,
    anonymization: Result<AnonymizationPorts, String>,
}

impl<D: ReadOnlyDatabase> SearchSchema<D> {
    pub fn new(database: Arc<D>, anonymization: Result<AnonymizationPorts, String>) -> Self {
        Self {
            database,
            anonymization,
        }
    }

    pub async fn execute(
        &self,
        request: SearchSchemaRequest,
    ) -> Result<SearchResponse, QueryingError> {
        let ports =
            self.anonymization
                .as_ref()
                .map_err(|reason| QueryingError::StoreUnavailable {
                    reason: reason.clone(),
                })?;
        let limits = QueryLimits::default();
        let output = self
            .database
            .read_catalog(
                &request.connection,
                &search_sql(&request.pattern),
                ExecutionLimits {
                    max_rows: SEARCH_LIMIT,
                    timeout_ms: limits.timeout_ms,
                },
            )
            .await
            .map_err(|failure| self.scrubbed(ports, failure, &limits))?;
        Ok(SearchResponse {
            matches: schema_matches(&output),
            truncated: output.more_rows,
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
            DatabaseFailure::Connect(text)
            | DatabaseFailure::Decode(text)
            | DatabaseFailure::ConnectFailed { reason: text, .. } => {
                ScrubbedText::Text(ports.anonymizer.free_text(text, &[]))
            }
            _ => ScrubbedText::Nothing,
        };
        QueryingError::from_failure(failure, limits.timeout_ms, scrubbed)
    }
}
