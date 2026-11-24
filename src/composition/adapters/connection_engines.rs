use std::sync::Arc;

use crate::contexts::connections::domain::constants::ENGINE_MONGODB;
use crate::contexts::connections::injector::ConnectionsContext;
use crate::contexts::querying::domain::repositories::ConnectionEngines;
use crate::contexts::querying::domain::types::Engine;

pub struct ConnectionEnginesAdapter {
    connections: Arc<ConnectionsContext>,
}

impl ConnectionEnginesAdapter {
    pub fn new(connections: Arc<ConnectionsContext>) -> Self {
        Self { connections }
    }
}

impl ConnectionEngines for ConnectionEnginesAdapter {
    async fn engine_of(&self, connection: &str) -> Option<Engine> {
        let discovery = self.connections.list_connections.execute().await.ok()?;
        let entry = discovery
            .connections
            .iter()
            .find(|entry| entry.name == connection)?;
        Some(if entry.engine == ENGINE_MONGODB {
            Engine::MongoDb
        } else {
            Engine::PostgreSql
        })
    }
}
