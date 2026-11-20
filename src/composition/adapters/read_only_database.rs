use std::sync::Arc;

use super::database_failure_mapping::database_failure;
use super::result_set_mapping::{execution_limits, query_output};
use crate::contexts::connections::injector::ConnectionsContext;
use crate::contexts::querying::domain::errors::DatabaseFailure;
use crate::contexts::querying::domain::repositories::ReadOnlyDatabase;
use crate::contexts::querying::domain::types::{ExecutionLimits, QueryOutput};

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
        sql: &str,
        limits: ExecutionLimits,
    ) -> Result<QueryOutput, DatabaseFailure> {
        self.connections
            .run_read_only_statement
            .execute(connection, sql, execution_limits(limits))
            .await
            .map(query_output)
            .map_err(database_failure)
    }

    async fn explain(
        &self,
        connection: &str,
        explain_sql: &str,
        limits: ExecutionLimits,
    ) -> Result<QueryOutput, DatabaseFailure> {
        self.connections
            .explain_statement
            .execute(connection, explain_sql, execution_limits(limits))
            .await
            .map(query_output)
            .map_err(database_failure)
    }

    async fn read_catalog(
        &self,
        connection: &str,
        catalog_sql: &str,
        limits: ExecutionLimits,
    ) -> Result<QueryOutput, DatabaseFailure> {
        self.connections
            .read_catalog
            .execute(connection, catalog_sql, execution_limits(limits))
            .await
            .map(query_output)
            .map_err(database_failure)
    }
}
