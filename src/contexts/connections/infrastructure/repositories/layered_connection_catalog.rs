use crate::contexts::connections::domain::repositories::ConnectionCatalog;
use crate::contexts::connections::domain::services::layer_sources;
use crate::contexts::connections::domain::types::Discovery;
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
        let dbeaver = load
            .read_dbeaver
            .then(|| self.dbeaver.discover_in(database));
        layer_sources(load.discovery, dbeaver, &load.empty_hint)
    }
}
