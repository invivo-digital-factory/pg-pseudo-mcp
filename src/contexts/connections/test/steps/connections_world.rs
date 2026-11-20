use std::path::PathBuf;

use cucumber::World;
use pg_pseudo_mcp::common::AppConfig;
use pg_pseudo_mcp::contexts::connections::injector::ConnectionsContext;
use serde_json::Value;

#[derive(World)]
#[world(init = Self::new)]
pub struct ConnectionsWorld {
    pub workspace: tempfile::TempDir,
    pub config_dir: tempfile::TempDir,
    pub known_hosts_dir: tempfile::TempDir,
    pub known_hosts: Option<PathBuf>,
    pub ssh_config: Option<PathBuf>,
    pub listing: Option<Value>,
    pub outcome: Option<Result<(), String>>,
}

impl std::fmt::Debug for ConnectionsWorld {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConnectionsWorld")
            .field("known_hosts", &self.known_hosts)
            .field("outcome", &self.outcome)
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
            listing: None,
            outcome: None,
        };
        std::fs::write(world.connections_file(), "").expect("empty connection file");
        world
    }

    pub fn connections_file(&self) -> PathBuf {
        self.config_dir.path().join("connections.toml")
    }

    pub fn context(&self) -> ConnectionsContext {
        ConnectionsContext::new(&AppConfig {
            dbeaver_home: Some(self.workspace.path().to_path_buf()),
            data_dir: None,
            ssh_known_hosts: self.known_hosts.clone(),
            ssh_config: self.ssh_config.clone(),
            connections_file: Some(self.connections_file()),
        })
    }

    pub fn write_known_hosts(&mut self, contents: &str) {
        let path = self.known_hosts_dir.path().join("known_hosts");
        std::fs::write(&path, contents).expect("known_hosts file");
        self.known_hosts = Some(path);
    }
}
