use std::sync::Arc;

use super::database_failure_mapping::database_failure;
use super::heavy_query_thresholds::heavy_query_thresholds;
use super::result_set_mapping::{database_listing, execution_limits, query_output};
use crate::contexts::connections::injector::ConnectionsContext;
use crate::contexts::querying::domain::errors::DatabaseFailure;
use crate::contexts::querying::domain::repositories::ReadOnlyDatabase;
use crate::contexts::querying::domain::types::{
    DatabaseListing, ExecutionLimits, HeavyQueryThresholds, QueryOutput,
};

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

    async fn read_catalog_cached(
        &self,
        connection: &str,
        database: Option<&str>,
        catalog_sql: &str,
        limits: ExecutionLimits,
    ) -> Result<QueryOutput, DatabaseFailure> {
        self.connections
            .read_catalog
            .execute_cached(connection, database, catalog_sql, execution_limits(limits))
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

    async fn heavy_query_thresholds(&self, connection: &str) -> Option<HeavyQueryThresholds> {
        heavy_query_thresholds(&self.connections, connection).await
    }

    async fn enum_labels(
        &self,
        connection: &str,
        database: Option<&str>,
    ) -> Result<Vec<String>, DatabaseFailure> {
        self.connections
            .read_enum_labels
            .execute(connection, database)
            .await
            .map_err(database_failure)
    }
}
