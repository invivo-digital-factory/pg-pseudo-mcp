use std::sync::Arc;

use crate::contexts::connections::domain::errors::ConnectionError;
use crate::contexts::connections::domain::repositories::ConnectionCatalog;
use crate::contexts::connections::domain::types::Discovery;

pub struct ListConnections {
    catalog: Arc<dyn ConnectionCatalog>,
}

impl ListConnections {
    pub fn new(catalog: Arc<dyn ConnectionCatalog>) -> Self {
        Self { catalog }
    }

    pub async fn execute(&self) -> Result<Discovery, ConnectionError> {
        let catalog = self.catalog.clone();
        tokio::task::spawn_blocking(move || catalog.discover())
            .await
            .map_err(|_| ConnectionError::discovery_interrupted())
    }
}
