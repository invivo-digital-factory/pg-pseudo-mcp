use std::sync::Arc;

use super::database_failure_mapping::database_failure;
use super::document_mapping::{collection_kind, stored_node};
use super::heavy_query_thresholds::heavy_query_thresholds;
use super::result_set_mapping::execution_limits;
use crate::contexts::connections::domain::types::{
    AggregateRequest, ConnectionStatus, DocumentOutput as StoredOutput, FindRequest,
};
use crate::contexts::connections::injector::ConnectionsContext;
use crate::contexts::querying::domain::errors::DatabaseFailure;
use crate::contexts::querying::domain::repositories::ReadOnlyDocumentStore;
use crate::contexts::querying::domain::types::{
    CollectionDetails, CollectionEntry, DocumentOutput, ExecutionLimits, FieldSample,
    GuardedAggregate, GuardedFind, HeavyQueryThresholds,
};

pub struct ReadOnlyDocumentStoreAdapter {
    connections: Arc<ConnectionsContext>,
}

impl ReadOnlyDocumentStoreAdapter {
    pub fn new(connections: Arc<ConnectionsContext>) -> Self {
        Self { connections }
    }
}

fn stored_find(database: &str, collection: &str, find: &GuardedFind) -> FindRequest {
    FindRequest {
        database: Some(database.to_string()),
        collection: collection.to_string(),
        filter: find.filter.clone(),
        projection: find.projection.clone(),
        sort: find.sort.clone(),
        skip: find.skip,
        limit: Some(find.limit),
        hint: find.hint.clone(),
        collation: find.collation.clone(),
    }
}

fn stored_aggregate(
    database: &str,
    collection: &str,
    aggregate: &GuardedAggregate,
) -> AggregateRequest {
    AggregateRequest {
        database: Some(database.to_string()),
        collection: collection.to_string(),
        pipeline: aggregate.pipeline.clone(),
        let_vars: aggregate.let_vars.clone(),
    }
}

fn document_output(output: StoredOutput) -> DocumentOutput {
    DocumentOutput {
        documents: output.documents.into_iter().map(stored_node).collect(),
        more_documents: output.more_documents,
    }
}

impl ReadOnlyDocumentStore for ReadOnlyDocumentStoreAdapter {
    async fn default_database(&self, connection: &str) -> Result<Option<String>, DatabaseFailure> {
        let discovery = self
            .connections
            .list_connections
            .execute()
            .await
            .map_err(database_failure)?;
        let Some(entry) = discovery
            .connections
            .iter()
            .find(|entry| entry.name == connection)
        else {
            return Err(DatabaseFailure::UnknownConnection {
                name: connection.to_string(),
                available: discovery
                    .connections
                    .iter()
                    .filter(|entry| entry.status == ConnectionStatus::Available)
                    .map(|entry| entry.name.clone())
                    .collect(),
            });
        };
        match &entry.status {
            ConnectionStatus::Available => Ok(entry
                .mongo
                .as_ref()
                .and_then(|params| params.database.clone())),
            ConnectionStatus::Unavailable { reason } => Err(DatabaseFailure::Unavailable {
                reason: reason.clone(),
            }),
            ConnectionStatus::Unsupported { reason } => Err(DatabaseFailure::Unsupported {
                reason: reason.clone(),
            }),
        }
    }

    async fn find(
        &self,
        connection: &str,
        database: &str,
        collection: &str,
        find: &GuardedFind,
        limits: ExecutionLimits,
    ) -> Result<DocumentOutput, DatabaseFailure> {
        let request = stored_find(database, collection, find);
        self.connections
            .run_find
            .execute(connection, &request, execution_limits(limits))
            .await
            .map(document_output)
            .map_err(database_failure)
    }

    async fn aggregate(
        &self,
        connection: &str,
        database: &str,
        collection: &str,
        aggregate: &GuardedAggregate,
        limits: ExecutionLimits,
    ) -> Result<DocumentOutput, DatabaseFailure> {
        let request = stored_aggregate(database, collection, aggregate);
        self.connections
            .run_aggregate
            .execute(connection, &request, execution_limits(limits))
            .await
            .map(document_output)
            .map_err(database_failure)
    }

    async fn explain_find(
        &self,
        connection: &str,
        database: &str,
        collection: &str,
        find: &GuardedFind,
        limits: ExecutionLimits,
    ) -> Result<serde_json::Value, DatabaseFailure> {
        self.connections
            .explain_documents
            .find(
                connection,
                &stored_find(database, collection, find),
                execution_limits(limits),
            )
            .await
            .map_err(database_failure)
    }

    async fn explain_aggregate(
        &self,
        connection: &str,
        database: &str,
        collection: &str,
        aggregate: &GuardedAggregate,
        limits: ExecutionLimits,
    ) -> Result<serde_json::Value, DatabaseFailure> {
        self.connections
            .explain_documents
            .aggregate(
                connection,
                &stored_aggregate(database, collection, aggregate),
                execution_limits(limits),
            )
            .await
            .map_err(database_failure)
    }

    async fn estimated_count(
        &self,
        connection: &str,
        database: &str,
        collection: &str,
    ) -> Result<Option<u64>, DatabaseFailure> {
        self.connections
            .read_collections
            .estimated_count(connection, Some(database), collection)
            .await
            .map_err(database_failure)
    }

    async fn heavy_query_thresholds(&self, connection: &str) -> Option<HeavyQueryThresholds> {
        heavy_query_thresholds(&self.connections, connection).await
    }

    async fn databases(&self, connection: &str) -> Result<Vec<String>, DatabaseFailure> {
        self.connections
            .list_document_databases
            .execute(connection)
            .await
            .map_err(database_failure)
    }

    async fn collections(
        &self,
        connection: &str,
        database: &str,
    ) -> Result<Vec<CollectionEntry>, DatabaseFailure> {
        self.connections
            .read_collections
            .list(connection, Some(database))
            .await
            .map(|collections| {
                collections
                    .into_iter()
                    .map(|collection| CollectionEntry {
                        name: collection.name,
                        kind: collection_kind(collection.kind),
                        estimated_count: collection.estimated_count,
                    })
                    .collect()
            })
            .map_err(database_failure)
    }

    async fn describe_collection(
        &self,
        connection: &str,
        database: &str,
        collection: &str,
    ) -> Result<CollectionDetails, DatabaseFailure> {
        self.connections
            .read_collections
            .describe(connection, Some(database), collection)
            .await
            .map(|description| CollectionDetails {
                estimated_count: description.estimated_count,
                indexes: description
                    .indexes
                    .into_iter()
                    .map(|index| index.keys)
                    .collect(),
                validator_fields: description.validator_fields,
            })
            .map_err(database_failure)
    }

    async fn sample_fields(
        &self,
        connection: &str,
        database: &str,
        collection: &str,
    ) -> Result<Vec<FieldSample>, DatabaseFailure> {
        self.connections
            .sample_fields
            .execute(connection, Some(database), collection)
            .await
            .map(|fields| {
                fields
                    .into_iter()
                    .map(|field| FieldSample {
                        path: field.path,
                        types: field.types,
                        presence: field.presence,
                    })
                    .collect()
            })
            .map_err(database_failure)
    }
}
