pub use crate::common::AppConfig;
use std::sync::Arc;

use crate::composition::adapters::ReadOnlyDatabaseAdapter;
use crate::composition::querying::querying_context;
use crate::contexts::anonymization::injector::{AnonymizationInjector, AnonymizationUseCases};
use crate::contexts::connections::injector::ConnectionsContext;
use crate::contexts::querying::injector::QueryingContext;

pub struct AppState {
    pub config: AppConfig,
    pub connections: Arc<ConnectionsContext>,
    pub querying: QueryingContext<ReadOnlyDatabaseAdapter>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let pseudonymizer = AnonymizationInjector::from_config(&config).use_cases();
        Self::with_pseudonymizer(config, pseudonymizer)
    }

    pub fn with_pseudonymizer(
        config: AppConfig,
        pseudonymizer: Result<AnonymizationUseCases, String>,
    ) -> Self {
        let connections = Arc::new(ConnectionsContext::new(&config));
        Self {
            querying: querying_context(Arc::clone(&connections), pseudonymizer),
            connections,
            config,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn empty_or_relative_directories_fall_back_to_defaults() {
        let config = AppConfig::from_vars(
            Some(OsString::new()),
            Some(OsString::from("relative")),
            Some(OsString::from("known_hosts")),
        );
        assert_eq!(config.dbeaver_home, None);
        assert_eq!(config.data_dir, None);
        assert_eq!(config.ssh_known_hosts, None);
        let config = AppConfig::from_vars(
            None,
            Some(OsString::from("/tmp/pg-pseudo")),
            Some(OsString::from("/tmp/known_hosts")),
        );
        assert_eq!(config.data_dir, Some(PathBuf::from("/tmp/pg-pseudo")));
        assert_eq!(
            config.ssh_known_hosts,
            Some(PathBuf::from("/tmp/known_hosts"))
        );
    }
}
