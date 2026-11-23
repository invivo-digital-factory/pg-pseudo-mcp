use std::sync::Arc;

use crate::contexts::connections::domain::errors::ConnectionError;
use crate::contexts::connections::domain::repositories::SessionPool;
use crate::contexts::connections::domain::services::{ENUM_LABELS_SQL, read_enum_labels};
use crate::contexts::connections::domain::types::ExecutionLimits;

const MAX_LABELS: usize = 10_000;
const TIMEOUT_MS: u64 = 2_000;

pub struct ReadEnumLabels<P: SessionPool> {
    pool: Arc<P>,
}

impl<P: SessionPool> ReadEnumLabels<P> {
    pub fn new(pool: Arc<P>) -> Self {
        Self { pool }
    }

    pub async fn execute(
        &self,
        connection: &str,
        database: Option<&str>,
    ) -> Result<Vec<String>, ConnectionError> {
        let output = self
            .pool
            .run_cached_read_only(
                connection,
                database,
                ENUM_LABELS_SQL,
                ExecutionLimits {
                    max_rows: MAX_LABELS,
                    timeout_ms: TIMEOUT_MS,
                },
            )
            .await?;
        read_enum_labels(&output).map_err(ConnectionError::Session)
    }
}
