use std::sync::Arc;

use crate::common::AppConfig;
use crate::contexts::connections::infrastructure::repositories::{
    DbeaverConnectionCatalog, PostgresSessionPool,
};
use crate::contexts::connections::use_cases::{
    ExplainStatement, ListConnections, ReadCatalog, RunReadOnlyStatement,
};

pub struct ConnectionsContext {
    pub list_connections: ListConnections,
    pub run_read_only_statement: RunReadOnlyStatement<PostgresSessionPool>,
    pub explain_statement: ExplainStatement<PostgresSessionPool>,
    pub read_catalog: ReadCatalog<PostgresSessionPool>,
}

impl ConnectionsContext {
    pub fn new(config: &AppConfig) -> Self {
        let catalog = Arc::new(DbeaverConnectionCatalog::new(config.dbeaver_home.clone()));
        let pool = Arc::new(PostgresSessionPool::new(
            catalog.clone(),
            config.ssh_known_hosts.clone(),
        ));
        Self {
            list_connections: ListConnections::new(catalog),
            run_read_only_statement: RunReadOnlyStatement::new(pool.clone()),
            explain_statement: ExplainStatement::new(pool.clone()),
            read_catalog: ReadCatalog::new(pool),
        }
    }
}
