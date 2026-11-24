use std::path::Path;

use crate::contexts::connections::domain::repositories::ConnectionCatalog;
use crate::contexts::connections::domain::services::layer_sources;
use crate::contexts::connections::domain::types::{ConnectionStatus, Discovery};
use crate::contexts::connections::infrastructure::data_sources::connection_file::DbeaverAccess;
use crate::contexts::connections::infrastructure::repositories::{
    DbeaverConnectionCatalog, FileConnectionCatalog,
};

pub struct LayeredConnectionCatalog {
    file: FileConnectionCatalog,
    dbeaver: DbeaverConnectionCatalog,
}

impl LayeredConnectionCatalog {
    pub fn new(file: FileConnectionCatalog, dbeaver: DbeaverConnectionCatalog) -> Self {
        Self { file, dbeaver }
    }
}

impl ConnectionCatalog for LayeredConnectionCatalog {
    fn discover_in(&self, database: Option<&str>) -> Discovery {
        let load = self.file.load(database);
        let dbeaver = match load.dbeaver {
            DbeaverAccess::Off => None,
            DbeaverAccess::Read => Some(self.dbeaver.discover_in(database)),
            DbeaverAccess::Blocked => {
                let reason = blocked_reason(load.path.as_deref());
                Some(block_entries(self.dbeaver.discover_in(database), &reason))
            }
        };
        layer_sources(load.discovery, dbeaver, &load.empty_hint)
    }
}

fn blocked_reason(path: Option<&Path>) -> String {
    let target = path.map_or_else(
        || "the connection file".to_string(),
        |path| path.display().to_string(),
    );
    format!(
        "the connection file {target} has a problem (see the hint); DBeaver connections stay unavailable until it is fixed"
    )
}

fn block_entries(mut discovery: Discovery, reason: &str) -> Discovery {
    for entry in &mut discovery.connections {
        entry.status = ConnectionStatus::Unavailable {
            reason: reason.to_string(),
        };
        entry.params = None;
        entry.mongo = None;
    }
    discovery
}
