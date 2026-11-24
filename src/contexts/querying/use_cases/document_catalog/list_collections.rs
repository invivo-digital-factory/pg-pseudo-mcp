use std::sync::Arc;

use crate::contexts::querying::domain::constants::catalog::{DEFAULT_LIST_LIMIT, MAX_LIST_LIMIT};
use crate::contexts::querying::domain::constants::mongo_policy::NO_SCHEMA_IN_MONGODB;
use crate::contexts::querying::domain::errors::{QueryingError, Refusal};
use crate::contexts::querying::domain::repositories::{DocumentPorts, ReadOnlyDocumentStore};
use crate::contexts::querying::domain::services::document_requests::{
    ports_of, resolve_database, scrubbed,
};
use crate::contexts::querying::domain::services::mongo_guard::check_names;
use crate::contexts::querying::domain::types::{
    CollectionLine, CollectionListResponse, ListTablesRequest, QueryLimits,
};

pub struct ListCollections<S: ReadOnlyDocumentStore> {
    store: Arc<S>,
    anonymization: Result<DocumentPorts, String>,
}

impl<S: ReadOnlyDocumentStore> ListCollections<S> {
    pub fn new(store: Arc<S>, anonymization: Result<DocumentPorts, String>) -> Self {
        Self {
            store,
            anonymization,
        }
    }

    pub async fn execute(
        &self,
        request: ListTablesRequest,
    ) -> Result<CollectionListResponse, QueryingError> {
        let ports = ports_of(&self.anonymization)?;
        if request
            .schema
            .as_deref()
            .is_some_and(|schema| !schema.trim().is_empty())
        {
            return Err(QueryingError::Refused(Refusal::new(
                "mongo_no_schema",
                NO_SCHEMA_IN_MONGODB,
            )));
        }
        let database = resolve_database(
            self.store.as_ref(),
            ports,
            &request.connection,
            request.database.as_deref(),
        )
        .await?;
        check_names(&database, None).map_err(QueryingError::Refused)?;
        let limit = request
            .limit
            .unwrap_or(DEFAULT_LIST_LIMIT)
            .clamp(1, MAX_LIST_LIMIT) as usize;
        let offset = request.offset.unwrap_or(0) as usize;
        let needle = request.name_like.as_deref().map(str::to_lowercase);
        let matching: Vec<_> = self
            .store
            .collections(&request.connection, &database)
            .await
            .map_err(|failure| scrubbed(ports, failure, QueryLimits::default().timeout_ms, &[]))?
            .into_iter()
            .filter(|entry| {
                needle
                    .as_deref()
                    .is_none_or(|needle| entry.name.to_lowercase().contains(needle))
            })
            .collect();
        let more = matching.len() > offset.saturating_add(limit);
        let collections = matching
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|entry| CollectionLine {
                name: entry.name,
                kind: entry.kind,
                estimated_documents: entry.estimated_count,
            })
            .collect();
        Ok(CollectionListResponse {
            database,
            collections,
            next_offset: more.then(|| (offset + limit) as u64),
        })
    }
}
