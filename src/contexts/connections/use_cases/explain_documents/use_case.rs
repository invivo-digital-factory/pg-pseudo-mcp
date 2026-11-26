use std::sync::Arc;

use serde_json::Value;

use crate::contexts::connections::domain::constants::AGGREGATE_WRITE_STAGE_REFUSED;
use crate::contexts::connections::domain::errors::ConnectionError;
use crate::contexts::connections::domain::repositories::DocumentSessionPool;
use crate::contexts::connections::domain::services::pipeline_writes;
use crate::contexts::connections::domain::types::{AggregateRequest, ExecutionLimits, FindRequest};

pub struct ExplainDocuments<P: DocumentSessionPool> {
    pool: Arc<P>,
}

impl<P: DocumentSessionPool> ExplainDocuments<P> {
    pub fn new(pool: Arc<P>) -> Self {
        Self { pool }
    }

    pub async fn find(
        &self,
        connection: &str,
        request: &FindRequest,
        limits: ExecutionLimits,
    ) -> Result<Value, ConnectionError> {
        self.pool.explain_find(connection, request, limits).await
    }

    pub async fn aggregate(
        &self,
        connection: &str,
        request: &AggregateRequest,
        limits: ExecutionLimits,
    ) -> Result<Value, ConnectionError> {
        if pipeline_writes(&request.pipeline) {
            return Err(ConnectionError::Unsupported {
                reason: AGGREGATE_WRITE_STAGE_REFUSED.to_string(),
            });
        }
        self.pool
            .explain_aggregate(connection, request, limits)
            .await
    }
}
