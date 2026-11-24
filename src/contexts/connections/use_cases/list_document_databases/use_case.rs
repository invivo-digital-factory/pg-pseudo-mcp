use std::sync::Arc;

use crate::contexts::connections::domain::errors::ConnectionError;
use crate::contexts::connections::domain::repositories::DocumentSessionPool;
use crate::contexts::connections::domain::services::visible_databases;

pub struct ListDocumentDatabases<P: DocumentSessionPool> {
    pool: Arc<P>,
}

impl<P: DocumentSessionPool> ListDocumentDatabases<P> {
    pub fn new(pool: Arc<P>) -> Self {
        Self { pool }
    }

    pub async fn execute(&self, connection: &str) -> Result<Vec<String>, ConnectionError> {
        Ok(visible_databases(
            self.pool.database_names(connection).await?,
        ))
    }
}
