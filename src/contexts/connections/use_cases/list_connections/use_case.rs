use std::sync::Arc;

use crate::contexts::connections::domain::repositories::ConnectionCatalog;
use crate::contexts::connections::domain::types::Discovery;

pub struct ListConnections {
    catalog: Arc<dyn ConnectionCatalog>,
}

impl ListConnections {
    pub fn new(catalog: Arc<dyn ConnectionCatalog>) -> Self {
        Self { catalog }
    }

    pub fn execute(&self) -> Discovery {
        self.catalog.discover()
    }
}
