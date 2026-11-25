use std::sync::Arc;

use super::field_labels::{field_entry, shown_path};
use crate::contexts::querying::domain::constants::mongo_policy::SAMPLE_HINT;
use crate::contexts::querying::domain::errors::QueryingError;
use crate::contexts::querying::domain::repositories::{
    ColumnPolicySource, DocumentPorts, ReadOnlyDocumentStore,
};
use crate::contexts::querying::domain::services::document_requests::{
    policy_of, ports_of, resolve_database, scrubbed,
};
use crate::contexts::querying::domain::services::mongo_guard::check_names;
use crate::contexts::querying::domain::types::{
    CollectionDescribeResponse, DescribeTableRequest, IndexKey, QueryLimits,
};

pub struct DescribeCollection<S: ReadOnlyDocumentStore> {
    store: Arc<S>,
    anonymization: Result<DocumentPorts, String>,
    policy: Option<Arc<dyn ColumnPolicySource>>,
}

impl<S: ReadOnlyDocumentStore> DescribeCollection<S> {
    pub fn new(store: Arc<S>, anonymization: Result<DocumentPorts, String>) -> Self {
        Self {
            store,
            anonymization,
            policy: None,
        }
    }

    pub fn with_column_policy(mut self, policy: Arc<dyn ColumnPolicySource>) -> Self {
        self.policy = Some(policy);
        self
    }

    pub async fn execute(
        &self,
        request: DescribeTableRequest,
    ) -> Result<CollectionDescribeResponse, QueryingError> {
        let ports = ports_of(&self.anonymization)?;
        let database = resolve_database(
            self.store.as_ref(),
            ports,
            &request.connection,
            request.database.as_deref(),
        )
        .await?;
        let collection = request.table;
        check_names(&database, Some(&collection)).map_err(QueryingError::Refused)?;
        let timeout_ms = QueryLimits::default().timeout_ms;
        let details = self
            .store
            .describe_collection(&request.connection, &database, &collection)
            .await
            .map_err(|failure| scrubbed(ports, failure, timeout_ms, &[]))?;
        let samples = self
            .store
            .sample_fields(&request.connection, &database, &collection)
            .await
            .map_err(|failure| scrubbed(ports, failure, timeout_ms, &[]))?;
        let policy = policy_of(self.policy.as_ref());
        let mut hints = Vec::new();
        let fields = samples
            .into_iter()
            .map(|sample| field_entry(ports, &policy, (&database, &collection), sample, &mut hints))
            .collect();
        let dotted = |path: &str| {
            let segments: Vec<String> = path.split('.').map(str::to_string).collect();
            shown_path(ports, &segments)
        };
        Ok(CollectionDescribeResponse {
            estimated_documents: details.estimated_count,
            sampled_documents_hint: SAMPLE_HINT,
            fields,
            column_policy_hints: hints,
            indexes: details
                .indexes
                .into_iter()
                .map(|keys| {
                    keys.into_iter()
                        .map(|(field, order)| IndexKey {
                            field: dotted(&field),
                            order: ports.anonymizer.key(&order),
                        })
                        .collect()
                })
                .collect(),
            validator_fields: details
                .validator_fields
                .iter()
                .map(|field| dotted(field))
                .collect(),
            database,
            collection,
        })
    }
}
