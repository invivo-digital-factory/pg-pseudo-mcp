pub use crate::common::AppConfig;
use crate::contexts::anonymization::injector::{AnonymizationInjector, AnonymizationUseCases};
use crate::contexts::connections::injector::ConnectionsContext;
use crate::tools::errors::{ErrorCategory, ToolError};

pub struct AppState {
    pub config: AppConfig,
    pub connections: ConnectionsContext,
    pseudonymizer: Result<AnonymizationUseCases, String>,
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
        Self {
            connections: ConnectionsContext::new(&config),
            config,
            pseudonymizer,
        }
    }

    pub fn pseudonymizer(&self) -> Result<AnonymizationUseCases, ToolError> {
        self.pseudonymizer.clone().map_err(|error| {
            ToolError::new(
                ErrorCategory::WithheldFailClosed,
                "pseudonym_store_unavailable",
                error,
            )
            .with_hint("ask the developer to run pg-pseudo-mcp outside the agent to repair the pseudonym store")
        })
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
