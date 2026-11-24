use std::sync::Arc;

use crate::contexts::connections::domain::errors::ConnectionError;
use crate::contexts::connections::domain::repositories::DocumentSessionPool;
use crate::contexts::connections::domain::types::{
    AggregateRequest, DocumentOutput, ExecutionLimits,
};

pub struct RunAggregate<P: DocumentSessionPool> {
    pool: Arc<P>,
}

impl<P: DocumentSessionPool> RunAggregate<P> {
    pub fn new(pool: Arc<P>) -> Self {
        Self { pool }
    }

    pub async fn execute(
        &self,
        connection: &str,
        request: &AggregateRequest,
        limits: ExecutionLimits,
    ) -> Result<DocumentOutput, ConnectionError> {
        self.pool.aggregate(connection, request, limits).await
    }
}
