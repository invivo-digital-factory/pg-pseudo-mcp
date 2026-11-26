use std::sync::Arc;
use std::time::Instant;

use super::shared::Execution;
use crate::contexts::querying::domain::errors::QueryingError;
use crate::contexts::querying::domain::repositories::{
    ColumnPolicySource, DocumentPorts, ReadOnlyDocumentStore,
};
use crate::contexts::querying::domain::services::document_clearances::DocumentCommand;
use crate::contexts::querying::domain::services::document_requests::{
    policy_of, ports_of, resolve_database, translations_for,
};
use crate::contexts::querying::domain::services::mongo_guard::inspect_find;
use crate::contexts::querying::domain::types::{DocumentsResponse, FindRequest, QueryLimits};

pub struct RunFind<S: ReadOnlyDocumentStore> {
    store: Arc<S>,
    anonymization: Result<DocumentPorts, String>,
    policy: Option<Arc<dyn ColumnPolicySource>>,
}

impl<S: ReadOnlyDocumentStore> RunFind<S> {
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
        mut request: FindRequest,
    ) -> Result<DocumentsResponse, QueryingError> {
        let started = Instant::now();
        let ports = ports_of(&self.anonymization)?;
        let database = resolve_database(
            self.store.as_ref(),
            ports,
            &request.connection,
            request.database.as_deref(),
        )
        .await?;
        request.database = Some(database.clone());
        let inspected = inspect_find(&request).map_err(QueryingError::Refused)?;
        let translations = translations_for(ports, inspected.literals());
        let (limits, clamps) =
            QueryLimits::from_request(request.max_rows, request.max_bytes, request.timeout_ms);
        let guarded = inspected.finish(&translations, &limits);
        Execution {
            store: self.store.as_ref(),
            ports,
            policy: policy_of(self.policy.as_ref()),
            connection: &request.connection,
            database: &database,
            command: DocumentCommand::Find,
            limits,
            clamps,
            cell_offset: request.cell_offset,
            confirm_heavy: request.confirm_heavy,
            started,
        }
        .run(guarded)
        .await
    }
}
