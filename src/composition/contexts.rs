use std::sync::Arc;

use super::adapters::ReadOnlyDatabaseAdapter;
use super::querying::querying_context;
use crate::common::AppConfig;
use crate::contexts::anonymization::injector::{AnonymizationInjector, AnonymizationUseCases};
use crate::contexts::connections::injector::ConnectionsContext;
use crate::contexts::querying::injector::QueryingContext;

pub struct Contexts {
    pub connections: Arc<ConnectionsContext>,
    pub querying: QueryingContext<ReadOnlyDatabaseAdapter>,
}

impl Contexts {
    pub fn new(config: &AppConfig) -> Self {
        let anonymization = AnonymizationInjector::from_config(config).use_cases();
        Self::with_anonymization(config, anonymization)
    }

    pub fn with_anonymization(
        config: &AppConfig,
        anonymization: Result<AnonymizationUseCases, String>,
    ) -> Self {
        let connections = Arc::new(ConnectionsContext::new(config));
        Self {
            querying: querying_context(Arc::clone(&connections), anonymization),
            connections,
        }
    }

    pub fn column_policy_warnings(&self) -> Vec<String> {
        self.querying.read_column_policy.execute().warnings
    }
}
