use std::sync::Arc;

use crate::contexts::querying::domain::errors::{DatabaseFailure, QueryingError, ScrubbedText};
use crate::contexts::querying::domain::repositories::{AnonymizationPorts, ReadOnlyDatabase};
use crate::contexts::querying::domain::types::{
    DatabaseListing, ListDatabasesRequest, QueryLimits,
};

pub struct ListDatabases<D: ReadOnlyDatabase> {
    database: Arc<D>,
    anonymization: Result<AnonymizationPorts, String>,
}

impl<D: ReadOnlyDatabase> ListDatabases<D> {
    pub fn new(database: Arc<D>, anonymization: Result<AnonymizationPorts, String>) -> Self {
        Self {
            database,
            anonymization,
        }
    }

    pub async fn execute(
        &self,
        request: ListDatabasesRequest,
    ) -> Result<DatabaseListing, QueryingError> {
        let ports =
            self.anonymization
                .as_ref()
                .map_err(|reason| QueryingError::StoreUnavailable {
                    reason: reason.clone(),
                })?;
        self.database
            .list_databases(&request.connection)
            .await
            .map_err(|failure| self.scrubbed(ports, failure))
    }

    fn scrubbed(&self, ports: &AnonymizationPorts, failure: DatabaseFailure) -> QueryingError {
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
        QueryingError::from_failure(failure, QueryLimits::default().timeout_ms, scrubbed)
    }
}
