use std::sync::Arc;

use crate::contexts::connections::domain::errors::ConnectionError;
use crate::contexts::connections::domain::repositories::DocumentSessionPool;
use crate::contexts::connections::domain::types::{DocumentOutput, ExecutionLimits, FindRequest};

pub struct RunFind<P: DocumentSessionPool> {
    pool: Arc<P>,
}

impl<P: DocumentSessionPool> RunFind<P> {
    pub fn new(pool: Arc<P>) -> Self {
        Self { pool }
    }

    pub async fn execute(
        &self,
        connection: &str,
        request: &FindRequest,
        limits: ExecutionLimits,
    ) -> Result<DocumentOutput, ConnectionError> {
        self.pool.find(connection, request, limits).await
    }
}
