use std::sync::Arc;

use crate::contexts::connections::domain::errors::ConnectionError;
use crate::contexts::connections::domain::repositories::SessionPool;
use crate::contexts::connections::domain::types::{ExecutionLimits, QueryOutput};

pub struct RunReadOnlyStatement<P: SessionPool> {
    pool: Arc<P>,
}

impl<P: SessionPool> RunReadOnlyStatement<P> {
    pub fn new(pool: Arc<P>) -> Self {
        Self { pool }
    }

    pub async fn execute(
        &self,
        connection: &str,
        database: Option<&str>,
        sql: &str,
        limits: ExecutionLimits,
    ) -> Result<QueryOutput, ConnectionError> {
        self.pool
            .run_read_only(connection, database, sql, limits)
            .await
    }
}
