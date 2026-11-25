use std::path::PathBuf;
use std::sync::Arc;

use crate::contexts::connections::domain::repositories::{
    AzureSignInTokenProvider, ConnectionCatalog,
};
use crate::contexts::connections::domain::types::Discovery;
use crate::contexts::connections::infrastructure::data_sources::dbeaver::discover_in;

pub struct DbeaverConnectionCatalog {
    dbeaver_home: Option<PathBuf>,
    tokens: Arc<dyn AzureSignInTokenProvider>,
    pgpass_file: Option<PathBuf>,
}

impl DbeaverConnectionCatalog {
    pub fn new(
        dbeaver_home: Option<PathBuf>,
        tokens: Arc<dyn AzureSignInTokenProvider>,
        pgpass_file: Option<PathBuf>,
    ) -> Self {
        Self {
            dbeaver_home,
            tokens,
            pgpass_file,
        }
    }
}

impl ConnectionCatalog for DbeaverConnectionCatalog {
    fn discover_in(&self, database: Option<&str>) -> Discovery {
        discover_in(
            self.dbeaver_home.as_deref(),
            self.tokens.as_ref(),
            self.pgpass_file.as_deref(),
            database,
        )
    }
}
