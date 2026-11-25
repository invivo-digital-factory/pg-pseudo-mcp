use std::sync::Arc;

use crate::contexts::querying::domain::errors::QueryingError;
use crate::contexts::querying::domain::repositories::{DocumentPorts, ReadOnlyDocumentStore};
use crate::contexts::querying::domain::services::document_requests::{ports_of, scrubbed};
use crate::contexts::querying::domain::services::mongo_guard::check_names;
use crate::contexts::querying::domain::types::{
    DatabaseListing, ListDatabasesRequest, QueryLimits,
};

pub struct ListMongoDatabases<S: ReadOnlyDocumentStore> {
    store: Arc<S>,
    anonymization: Result<DocumentPorts, String>,
}

impl<S: ReadOnlyDocumentStore> ListMongoDatabases<S> {
    pub fn new(store: Arc<S>, anonymization: Result<DocumentPorts, String>) -> Self {
        Self {
            store,
            anonymization,
        }
    }

    pub async fn execute(
        &self,
        request: ListDatabasesRequest,
    ) -> Result<DatabaseListing, QueryingError> {
        let ports = ports_of(&self.anonymization)?;
        let timeout_ms = QueryLimits::default().timeout_ms;
        let default = self
            .store
            .default_database(&request.connection)
            .await
            .map_err(|failure| scrubbed(ports, failure, timeout_ms, &[]))?
            .filter(|name| check_names(name, None).is_ok());
        let databases = self
            .store
            .databases(&request.connection)
            .await
            .map_err(|failure| scrubbed(ports, failure, timeout_ms, &[]))?;
        Ok(DatabaseListing {
            databases,
            default,
            truncated: false,
        })
    }
}
