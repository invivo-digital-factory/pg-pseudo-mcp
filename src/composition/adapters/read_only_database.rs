use std::sync::Arc;

use super::database_failure_mapping::database_failure;
use super::result_set_mapping::{database_listing, execution_limits, query_output};
use crate::contexts::connections::injector::ConnectionsContext;
use crate::contexts::querying::domain::errors::DatabaseFailure;
use crate::contexts::querying::domain::repositories::ReadOnlyDatabase;
use crate::contexts::querying::domain::types::{DatabaseListing, ExecutionLimits, QueryOutput};

pub struct ReadOnlyDatabaseAdapter {
    connections: Arc<ConnectionsContext>,
}

impl ReadOnlyDatabaseAdapter {
    pub fn new(connections: Arc<ConnectionsContext>) -> Self {
        Self { connections }
    }
}

impl ReadOnlyDatabase for ReadOnlyDatabaseAdapter {
    async fn run(
        &self,
        connection: &str,
        database: Option<&str>,
        sql: &str,
        limits: ExecutionLimits,
    ) -> Result<QueryOutput, DatabaseFailure> {
        self.connections
            .run_read_only_statement
            .execute(connection, database, sql, execution_limits(limits))
            .await
            .map(query_output)
            .map_err(database_failure)
    }

    async fn explain(
        &self,
        connection: &str,
        database: Option<&str>,
        explain_sql: &str,
        limits: ExecutionLimits,
    ) -> Result<QueryOutput, DatabaseFailure> {
        self.connections
            .explain_statement
            .execute(connection, database, explain_sql, execution_limits(limits))
            .await
            .map(query_output)
            .map_err(database_failure)
    }

    async fn read_catalog(
        &self,
        connection: &str,
        database: Option<&str>,
        catalog_sql: &str,
        limits: ExecutionLimits,
    ) -> Result<QueryOutput, DatabaseFailure> {
        self.connections
            .read_catalog
            .execute(connection, database, catalog_sql, execution_limits(limits))
            .await
            .map(query_output)
            .map_err(database_failure)
    }

    async fn list_databases(&self, connection: &str) -> Result<DatabaseListing, DatabaseFailure> {
        self.connections
            .list_databases
            .execute(connection)
            .await
            .map(database_listing)
            .map_err(database_failure)
    }
}
