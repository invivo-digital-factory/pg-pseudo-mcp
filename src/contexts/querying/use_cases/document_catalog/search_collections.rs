use std::sync::Arc;

use super::field_labels::shown_path;
use crate::contexts::querying::domain::constants::catalog::SEARCH_LIMIT;
use crate::contexts::querying::domain::constants::mongo_policy::MAX_SEARCHED_COLLECTIONS;
use crate::contexts::querying::domain::errors::QueryingError;
use crate::contexts::querying::domain::repositories::{DocumentPorts, ReadOnlyDocumentStore};
use crate::contexts::querying::domain::services::document_requests::{
    ports_of, resolve_database, scrubbed,
};
use crate::contexts::querying::domain::services::mongo_guard::check_names;
use crate::contexts::querying::domain::types::{
    QueryLimits, SchemaMatch, SearchResponse, SearchSchemaRequest,
};

pub struct SearchCollections<S: ReadOnlyDocumentStore> {
    store: Arc<S>,
    anonymization: Result<DocumentPorts, String>,
}

impl<S: ReadOnlyDocumentStore> SearchCollections<S> {
    pub fn new(store: Arc<S>, anonymization: Result<DocumentPorts, String>) -> Self {
        Self {
            store,
            anonymization,
        }
    }

    pub async fn execute(
        &self,
        request: SearchSchemaRequest,
    ) -> Result<SearchResponse, QueryingError> {
        let ports = ports_of(&self.anonymization)?;
        let database = resolve_database(
            self.store.as_ref(),
            ports,
            &request.connection,
            request.database.as_deref(),
        )
        .await?;
        check_names(&database, None).map_err(QueryingError::Refused)?;
        let needle = request.pattern.to_lowercase();
        let collections = self
            .store
            .collections(&request.connection, &database)
            .await
            .map_err(|failure| scrubbed(ports, failure, QueryLimits::default().timeout_ms, &[]))?;
        let mut matches = Vec::new();
        let mut truncated = collections.len() > MAX_SEARCHED_COLLECTIONS;
        for collection in collections.iter().take(MAX_SEARCHED_COLLECTIONS) {
            if collection.name.to_lowercase().contains(&needle) {
                matches.push(SchemaMatch {
                    schema: database.clone(),
                    table: collection.name.clone(),
                    column: None,
                    type_name: None,
                });
            }
            let Ok(fields) = self
                .store
                .sample_fields(&request.connection, &database, &collection.name)
                .await
            else {
                continue;
            };
            for field in fields {
                let path = shown_path(ports, &field.path);
                if path.to_lowercase().contains(&needle) {
                    matches.push(SchemaMatch {
                        schema: database.clone(),
                        table: collection.name.clone(),
                        column: Some(path),
                        type_name: Some(field.types.join("|")),
                    });
                }
            }
            if matches.len() > SEARCH_LIMIT {
                break;
            }
        }
        if matches.len() > SEARCH_LIMIT {
            matches.truncate(SEARCH_LIMIT);
            truncated = true;
        }
        Ok(SearchResponse { matches, truncated })
    }
}
