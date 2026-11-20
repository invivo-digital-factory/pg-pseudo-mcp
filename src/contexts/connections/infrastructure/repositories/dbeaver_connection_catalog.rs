use std::path::PathBuf;

use crate::contexts::connections::domain::repositories::ConnectionCatalog;
use crate::contexts::connections::domain::types::Discovery;
use crate::contexts::connections::infrastructure::data_sources::dbeaver::discover;

pub struct DbeaverConnectionCatalog {
    dbeaver_home: Option<PathBuf>,
}

impl DbeaverConnectionCatalog {
    pub fn new(dbeaver_home: Option<PathBuf>) -> Self {
        Self { dbeaver_home }
    }
}

impl ConnectionCatalog for DbeaverConnectionCatalog {
    fn discover(&self) -> Discovery {
        discover(self.dbeaver_home.as_deref())
    }
}
