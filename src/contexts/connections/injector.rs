use std::sync::Arc;

use crate::common::AppConfig;
use crate::contexts::connections::domain::repositories::{
    AzureSignInTokenProvider, CommandPasswordProvider,
};
use crate::contexts::connections::infrastructure::data_sources::azure::{AzureCli, SystemClock};
use crate::contexts::connections::infrastructure::data_sources::password_command::ProcessPasswordCommand;
use crate::contexts::connections::infrastructure::repositories::{
    CachedAzureSignInTokens, CachedCommandPasswords, DbeaverConnectionCatalog, PostgresSessionPool,
};
use crate::contexts::connections::use_cases::{
    ExplainStatement, ListConnections, ListDatabases, ReadCatalog, RunReadOnlyStatement,
};

pub struct ConnectionsContext {
    pub list_connections: ListConnections,
    pub run_read_only_statement: RunReadOnlyStatement<PostgresSessionPool>,
    pub explain_statement: ExplainStatement<PostgresSessionPool>,
    pub read_catalog: ReadCatalog<PostgresSessionPool>,
    pub list_databases: ListDatabases<PostgresSessionPool>,
}

impl ConnectionsContext {
    pub fn new(config: &AppConfig) -> Self {
        Self::with_azure_cli(config, AzureCli::from_path())
    }

    #[cfg(test)]
    pub(crate) fn with_azure_command(
        config: &AppConfig,
        command: Option<std::path::PathBuf>,
    ) -> Self {
        Self::with_azure_cli(config, AzureCli::new(command))
    }

    fn with_azure_cli(config: &AppConfig, cli: AzureCli) -> Self {
        let tokens: Arc<dyn AzureSignInTokenProvider> = Arc::new(CachedAzureSignInTokens::new(
            Arc::new(cli),
            Arc::new(SystemClock),
        ));
        let catalog = Arc::new(DbeaverConnectionCatalog::new(
            config.dbeaver_home.clone(),
            tokens.clone(),
        ));
        let commands: Arc<dyn CommandPasswordProvider> = Arc::new(CachedCommandPasswords::new(
            Arc::new(ProcessPasswordCommand::new()),
        ));
        let pool = Arc::new(PostgresSessionPool::new(
            catalog.clone(),
            config.ssh_known_hosts.clone(),
            tokens,
            commands,
        ));
        Self {
            list_connections: ListConnections::new(catalog),
            run_read_only_statement: RunReadOnlyStatement::new(pool.clone()),
            explain_statement: ExplainStatement::new(pool.clone()),
            read_catalog: ReadCatalog::new(pool.clone()),
            list_databases: ListDatabases::new(pool),
        }
    }
}
