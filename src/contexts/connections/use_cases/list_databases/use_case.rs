use std::sync::Arc;

use crate::contexts::connections::domain::errors::ConnectionError;
use crate::contexts::connections::domain::repositories::SessionPool;
use crate::contexts::connections::domain::services::{LIST_DATABASES_SQL, read_database_listing};
use crate::contexts::connections::domain::types::{DatabaseListing, ExecutionLimits};

const MAX_DATABASES: usize = 500;
const TIMEOUT_MS: u64 = 10_000;

pub struct ListDatabases<P: SessionPool> {
    pool: Arc<P>,
}

impl<P: SessionPool> ListDatabases<P> {
    pub fn new(pool: Arc<P>) -> Self {
        Self { pool }
    }

    pub async fn execute(&self, connection: &str) -> Result<DatabaseListing, ConnectionError> {
        let output = self
            .pool
            .run_read_only(
                connection,
                None,
                LIST_DATABASES_SQL,
                ExecutionLimits {
                    max_rows: MAX_DATABASES,
                    timeout_ms: TIMEOUT_MS,
                },
            )
            .await?;
        read_database_listing(&output).map_err(ConnectionError::Session)
    }
}
