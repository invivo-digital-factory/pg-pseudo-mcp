use std::sync::Arc;

use crate::contexts::connections::domain::errors::ConnectionError;
use crate::contexts::connections::domain::repositories::DocumentSessionPool;
use crate::contexts::connections::domain::types::SampledField;

pub struct SampleFields<P: DocumentSessionPool> {
    pool: Arc<P>,
}

impl<P: DocumentSessionPool> SampleFields<P> {
    pub fn new(pool: Arc<P>) -> Self {
        Self { pool }
    }

    pub async fn execute(
        &self,
        connection: &str,
        database: Option<&str>,
        collection: &str,
    ) -> Result<Vec<SampledField>, ConnectionError> {
        self.pool
            .sample_fields(connection, database, collection)
            .await
    }
}
