use std::sync::Arc;

use crate::common::AppConfig;
use crate::contexts::connections::domain::repositories::{
    AzureSignInTokenProvider, CommandPasswordProvider,
};
use crate::contexts::connections::infrastructure::data_sources::azure::{AzureCli, SystemClock};
use crate::contexts::connections::infrastructure::data_sources::connection_file::{
    TomlConnectionFileWriter, resolve_user_config_file,
};
use crate::contexts::connections::infrastructure::data_sources::dbeaver::DbeaverWorkspaceDefinitions;
use crate::contexts::connections::infrastructure::data_sources::openssh::{
    LocalEnvironment, OpenSshAliasDirectory, OpenSshResolver, SshCommand,
};
use crate::contexts::connections::infrastructure::data_sources::password_command::ProcessPasswordCommand;
use crate::contexts::connections::infrastructure::data_sources::pgpass;
use crate::contexts::connections::infrastructure::repositories::{
    CachedAzureSignInTokens, CachedCommandPasswords, DbeaverConnectionCatalog,
    FileConnectionCatalog, LayeredConnectionCatalog, MongoSessionPool, PostgresSessionPool,
    TomlColumnPolicyFiles,
};
use crate::contexts::connections::presentation::ImportDbeaverHandler;
use crate::contexts::connections::use_cases::{
    ExplainStatement, ImportDbeaverConnections, ListConnections, ListDatabases,
    ListDocumentDatabases, ReadCatalog, ReadCollections, ReadColumnPolicyFiles, ReadEnumLabels,
    RunAggregate, RunFind, RunReadOnlyStatement, SampleFields,
};

const COLUMN_POLICY_FILE: &str = "columns.toml";

pub struct ConnectionsContext {
    pub list_connections: ListConnections,
    pub run_read_only_statement: RunReadOnlyStatement<PostgresSessionPool>,
    pub explain_statement: ExplainStatement<PostgresSessionPool>,
    pub read_catalog: ReadCatalog<PostgresSessionPool>,
    pub list_databases: ListDatabases<PostgresSessionPool>,
    pub read_enum_labels: ReadEnumLabels<PostgresSessionPool>,
    pub read_column_policy_files: ReadColumnPolicyFiles,
    pub run_find: RunFind<MongoSessionPool>,
    pub run_aggregate: RunAggregate<MongoSessionPool>,
    pub list_document_databases: ListDocumentDatabases<MongoSessionPool>,
    pub read_collections: ReadCollections<MongoSessionPool>,
    pub sample_fields: SampleFields<MongoSessionPool>,
}

pub fn import_dbeaver_handler(config: &AppConfig) -> ImportDbeaverHandler {
    ImportDbeaverHandler::new(import_dbeaver_use_case(config))
}

pub fn import_dbeaver_use_case(config: &AppConfig) -> ImportDbeaverConnections {
    let environment = LocalEnvironment::from_process();
    let aliases = OpenSshAliasDirectory::new(
        Arc::new(SshCommand::from_path(config.ssh_config.clone())),
        config.ssh_config.clone(),
        environment.home,
    );
    ImportDbeaverConnections::new(
        Arc::new(DbeaverWorkspaceDefinitions::new(
            config.dbeaver_home.clone(),
        )),
        Arc::new(TomlConnectionFileWriter::new(
            config.connections_file.clone(),
        )),
        Arc::new(aliases),
    )
}

impl ConnectionsContext {
    pub fn new(config: &AppConfig) -> Self {
        Self::with_azure_cli(config, AzureCli::from_path(), pgpass::default_path())
    }

    #[cfg(test)]
    pub(crate) fn with_azure_command(
        config: &AppConfig,
        command: Option<std::path::PathBuf>,
        pgpass_file: Option<std::path::PathBuf>,
    ) -> Self {
        Self::with_azure_cli(config, AzureCli::new(command), pgpass_file)
    }

    fn with_azure_cli(
        config: &AppConfig,
        cli: AzureCli,
        pgpass_file: Option<std::path::PathBuf>,
    ) -> Self {
        let tokens: Arc<dyn AzureSignInTokenProvider> = Arc::new(CachedAzureSignInTokens::new(
            Arc::new(cli),
            Arc::new(SystemClock),
        ));
        let environment = LocalEnvironment::from_process();
        let dump = Arc::new(SshCommand::from_path(config.ssh_config.clone()));
        let aliases = Arc::new(OpenSshAliasDirectory::new(
            dump.clone(),
            config.ssh_config.clone(),
            environment.home.clone(),
        ));
        let resolver = Arc::new(OpenSshResolver::new(dump, environment, aliases));
        let catalog = Arc::new(LayeredConnectionCatalog::new(
            FileConnectionCatalog::new(
                config.connections_file.clone(),
                tokens.clone(),
                pgpass_file.clone(),
                resolver,
            ),
            DbeaverConnectionCatalog::new(config.dbeaver_home.clone(), tokens.clone(), pgpass_file),
        ));
        let commands: Arc<dyn CommandPasswordProvider> = Arc::new(CachedCommandPasswords::new(
            Arc::new(ProcessPasswordCommand::new()),
        ));
        let pool = Arc::new(PostgresSessionPool::new(
            catalog.clone(),
            config.ssh_known_hosts.clone(),
            tokens,
            commands.clone(),
        ));
        let documents = Arc::new(MongoSessionPool::new(
            catalog.clone(),
            config.ssh_known_hosts.clone(),
            commands,
        ));
        Self {
            list_connections: ListConnections::new(catalog),
            run_read_only_statement: RunReadOnlyStatement::new(pool.clone()),
            explain_statement: ExplainStatement::new(pool.clone()),
            read_catalog: ReadCatalog::new(pool.clone()),
            list_databases: ListDatabases::new(pool.clone()),
            read_enum_labels: ReadEnumLabels::new(pool),
            read_column_policy_files: ReadColumnPolicyFiles::new(Arc::new(
                TomlColumnPolicyFiles::new(
                    config.columns_file.clone(),
                    resolve_user_config_file(
                        COLUMN_POLICY_FILE,
                        config.xdg_config_home.clone(),
                        config.home_dir.as_deref(),
                    )
                    .map(|file| file.path),
                ),
            )),
            run_find: RunFind::new(documents.clone()),
            run_aggregate: RunAggregate::new(documents.clone()),
            list_document_databases: ListDocumentDatabases::new(documents.clone()),
            read_collections: ReadCollections::new(documents.clone()),
            sample_fields: SampleFields::new(documents),
        }
    }
}
