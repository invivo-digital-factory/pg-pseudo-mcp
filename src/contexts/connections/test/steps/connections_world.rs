use std::path::PathBuf;
use std::sync::Arc;

use cucumber::World;
use scarif_database_mcp::common::AppConfig;
use scarif_database_mcp::contexts::connections::domain::errors::ConnectionError;
use scarif_database_mcp::contexts::connections::domain::repositories::{
    AzureSignInTokenProvider, CommandPasswordProvider, ConnectionCatalog,
};
use scarif_database_mcp::contexts::connections::domain::types::Discovery;
use scarif_database_mcp::contexts::connections::infrastructure::data_sources::azure::{
    AzureCli, SystemClock,
};
use scarif_database_mcp::contexts::connections::infrastructure::data_sources::openssh::{
    LocalEnvironment, OpenSshAliasDirectory, OpenSshResolver, SshCommand,
};
use scarif_database_mcp::contexts::connections::infrastructure::data_sources::password_command::ProcessPasswordCommand;
use scarif_database_mcp::contexts::connections::infrastructure::data_sources::socks::ProxyCredentials;
use scarif_database_mcp::contexts::connections::infrastructure::repositories::{
    CachedAzureSignInTokens, CachedCommandPasswords, DbeaverConnectionCatalog,
    FileConnectionCatalog, LayeredConnectionCatalog, MongoSessionPool, PostgresSessionPool,
};
use scarif_database_mcp::contexts::connections::use_cases::{
    ListConnections, ReadCatalog, RunFind, RunReadOnlyStatement,
};
use serde_json::Value;

use crate::connections_steps::front_socks5::SocksState;

pub struct Wiring {
    pub catalog: Arc<LayeredConnectionCatalog>,
    pub list_connections: ListConnections,
    pub run_read_only_statement: RunReadOnlyStatement<PostgresSessionPool>,
    pub read_catalog: ReadCatalog<PostgresSessionPool>,
    pub run_find: RunFind<MongoSessionPool>,
}

#[derive(World)]
#[world(init = Self::new)]
pub struct ConnectionsWorld {
    pub workspace: tempfile::TempDir,
    pub config_dir: tempfile::TempDir,
    pub known_hosts_dir: tempfile::TempDir,
    pub known_hosts: Option<PathBuf>,
    pub ssh_config: Option<PathBuf>,
    pub ssh_installed: bool,
    pub azure_cli: Option<PathBuf>,
    pub listing: Option<Value>,
    pub outcome: Option<Result<(), String>>,
    pub failure: Option<ConnectionError>,
    pub import_outcome: Option<Result<String, String>>,
    pub probe: Option<Value>,
    pub discovery: Option<Discovery>,
    pub proxy: Option<ProxyCredentials>,
    pub socks: Option<SocksState>,
    wiring: Option<Arc<Wiring>>,
}

impl std::fmt::Debug for ConnectionsWorld {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConnectionsWorld")
            .field("known_hosts", &self.known_hosts)
            .field("outcome", &self.outcome)
            .field("probe", &self.probe)
            .finish_non_exhaustive()
    }
}

impl ConnectionsWorld {
    fn new() -> Self {
        let world = Self {
            workspace: tempfile::tempdir().expect("workspace dir"),
            config_dir: tempfile::tempdir().expect("config dir"),
            known_hosts_dir: tempfile::tempdir().expect("known_hosts dir"),
            known_hosts: None,
            ssh_config: None,
            ssh_installed: true,
            azure_cli: None,
            listing: None,
            outcome: None,
            failure: None,
            import_outcome: None,
            probe: None,
            discovery: None,
            proxy: None,
            socks: None,
            wiring: None,
        };
        std::fs::write(world.connections_file(), "").expect("empty connection file");
        let ssh_config = world.config_dir.path().join("ssh_config");
        std::fs::write(&ssh_config, "").expect("empty ssh config");
        Self {
            ssh_config: Some(ssh_config),
            ..world
        }
    }

    pub fn connections_file(&self) -> PathBuf {
        self.config_dir.path().join("connections.toml")
    }

    pub fn pgpass_file(&self) -> PathBuf {
        self.config_dir.path().join("pgpass")
    }

    pub fn config(&self) -> AppConfig {
        AppConfig {
            dbeaver_home: Some(self.workspace.path().to_path_buf()),
            data_dir: None,
            ssh_known_hosts: self.known_hosts.clone(),
            ssh_config: self.ssh_config.clone(),
            connections_file: Some(self.connections_file()),
            columns_file: None,
            xdg_config_home: None,
            home_dir: None,
            data_home: None,
        }
    }

    pub fn context(&mut self) -> Arc<Wiring> {
        if let Some(wiring) = &self.wiring {
            return wiring.clone();
        }
        let wiring = Arc::new(self.wire());
        self.wiring = Some(wiring.clone());
        wiring
    }

    fn wire(&self) -> Wiring {
        let tokens: Arc<dyn AzureSignInTokenProvider> = Arc::new(CachedAzureSignInTokens::new(
            Arc::new(AzureCli::new(self.azure_cli.clone())),
            Arc::new(SystemClock),
        ));
        let ssh = if self.ssh_installed {
            SshCommand::from_path(self.ssh_config.clone())
        } else {
            SshCommand::new(None, self.ssh_config.clone())
        };
        let environment = LocalEnvironment::from_process();
        let dump = Arc::new(ssh);
        let aliases = Arc::new(OpenSshAliasDirectory::new(
            dump.clone(),
            self.ssh_config.clone(),
            environment.home.clone(),
        ));
        let resolver = Arc::new(OpenSshResolver::new(dump, environment, aliases));
        let catalog = Arc::new(LayeredConnectionCatalog::new(
            FileConnectionCatalog::new(
                Some(self.connections_file()),
                tokens.clone(),
                Some(self.pgpass_file()),
                resolver,
            ),
            DbeaverConnectionCatalog::new(
                Some(self.workspace.path().to_path_buf()),
                tokens.clone(),
                Some(self.pgpass_file()),
            ),
        ));
        let shared: Arc<dyn ConnectionCatalog> = catalog.clone();
        let commands: Arc<dyn CommandPasswordProvider> = Arc::new(CachedCommandPasswords::new(
            Arc::new(ProcessPasswordCommand::new()),
        ));
        let pool = Arc::new(PostgresSessionPool::new(
            shared.clone(),
            self.known_hosts.clone(),
            tokens,
            commands.clone(),
        ));
        let documents = Arc::new(MongoSessionPool::new(
            shared.clone(),
            self.known_hosts.clone(),
            commands,
        ));
        Wiring {
            catalog,
            list_connections: ListConnections::new(shared),
            run_read_only_statement: RunReadOnlyStatement::new(pool.clone()),
            read_catalog: ReadCatalog::new(pool),
            run_find: RunFind::new(documents),
        }
    }

    pub fn write_known_hosts(&mut self, contents: &str) {
        let path = self.known_hosts_dir.path().join("known_hosts");
        std::fs::write(&path, contents).expect("known_hosts file");
        self.known_hosts = Some(path);
    }
}
