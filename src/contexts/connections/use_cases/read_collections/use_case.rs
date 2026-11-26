use std::sync::Arc;

use crate::contexts::connections::domain::errors::ConnectionError;
use crate::contexts::connections::domain::repositories::DocumentSessionPool;
use crate::contexts::connections::domain::types::{CollectionDescription, CollectionInfo};

pub struct ReadCollections<P: DocumentSessionPool> {
    pool: Arc<P>,
}

impl<P: DocumentSessionPool> ReadCollections<P> {
    pub fn new(pool: Arc<P>) -> Self {
        Self { pool }
    }

    pub async fn list(
        &self,
        connection: &str,
        database: Option<&str>,
    ) -> Result<Vec<CollectionInfo>, ConnectionError> {
        self.pool.collections(connection, database).await
    }

    pub async fn estimated_count(
        &self,
        connection: &str,
        database: Option<&str>,
        collection: &str,
    ) -> Result<Option<u64>, ConnectionError> {
        self.pool
            .estimated_count(connection, database, collection)
            .await
    }

    pub async fn describe(
        &self,
        connection: &str,
        database: Option<&str>,
        collection: &str,
    ) -> Result<CollectionDescription, ConnectionError> {
        self.pool
            .describe_collection(connection, database, collection)
            .await?
            .ok_or_else(|| ConnectionError::UnknownCollection {
                requested: collection.to_string(),
            })
    }
}
