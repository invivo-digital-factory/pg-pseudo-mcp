use std::sync::Arc;

use crate::contexts::connections::domain::errors::ConnectionError;
use crate::contexts::connections::domain::repositories::SessionPool;
use crate::contexts::connections::domain::types::{ExecutionLimits, QueryOutput};

pub struct ExplainStatement<P: SessionPool> {
    pool: Arc<P>,
}

impl<P: SessionPool> ExplainStatement<P> {
    pub fn new(pool: Arc<P>) -> Self {
        Self { pool }
    }

    pub async fn execute(
        &self,
        connection: &str,
        database: Option<&str>,
        explain_sql: &str,
        limits: ExecutionLimits,
    ) -> Result<QueryOutput, ConnectionError> {
        self.pool
            .run_read_only(connection, database, explain_sql, limits)
            .await
    }
}
