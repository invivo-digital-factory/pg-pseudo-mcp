use std::sync::Arc;

pub use crate::common::AppConfig;
use crate::contexts::connections::injector::ConnectionsContext;
use crate::pseudo::Pseudonymizer;
use crate::tools::errors::{ErrorCategory, ToolError};

pub struct AppState {
    pub config: AppConfig,
    pub connections: ConnectionsContext,
    pseudonymizer: Result<Arc<Pseudonymizer>, String>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let pseudonymizer = Pseudonymizer::open_default(&config)
            .map(Arc::new)
            .map_err(|error| error.to_string());
        if let Err(error) = &pseudonymizer {
            tracing::error!(%error, "pseudonym store unavailable; data tools will refuse to answer");
        }
        Self::with_pseudonymizer(config, pseudonymizer)
    }

    pub fn with_pseudonymizer(
        config: AppConfig,
        pseudonymizer: Result<Arc<Pseudonymizer>, String>,
    ) -> Self {
        Self {
            connections: ConnectionsContext::new(&config),
            config,
            pseudonymizer,
        }
    }

    pub fn pseudonymizer(&self) -> Result<Arc<Pseudonymizer>, ToolError> {
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
