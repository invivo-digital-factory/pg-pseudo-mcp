use std::future::Future;

use crate::contexts::querying::domain::errors::DatabaseFailure;
use crate::contexts::querying::domain::types::{
    DatabaseListing, ExecutionLimits, HeavyQueryThresholds, QueryOutput,
};

pub trait ReadOnlyDatabase: Send + Sync {
    fn run(
        &self,
        connection: &str,
        database: Option<&str>,
        sql: &str,
        limits: ExecutionLimits,
    ) -> impl Future<Output = Result<QueryOutput, DatabaseFailure>> + Send;

    fn explain(
        &self,
        connection: &str,
        database: Option<&str>,
        explain_sql: &str,
        limits: ExecutionLimits,
    ) -> impl Future<Output = Result<QueryOutput, DatabaseFailure>> + Send;

    fn read_catalog(
        &self,
        connection: &str,
        database: Option<&str>,
        catalog_sql: &str,
        limits: ExecutionLimits,
    ) -> impl Future<Output = Result<QueryOutput, DatabaseFailure>> + Send;

    fn read_catalog_cached(
        &self,
        connection: &str,
        database: Option<&str>,
        catalog_sql: &str,
        limits: ExecutionLimits,
    ) -> impl Future<Output = Result<QueryOutput, DatabaseFailure>> + Send;

    fn list_databases(
        &self,
        connection: &str,
    ) -> impl Future<Output = Result<DatabaseListing, DatabaseFailure>> + Send;

    fn heavy_query_thresholds(
        &self,
        connection: &str,
    ) -> impl Future<Output = Option<HeavyQueryThresholds>> + Send;

    fn enum_labels(
        &self,
        connection: &str,
        database: Option<&str>,
    ) -> impl Future<Output = Result<Vec<String>, DatabaseFailure>> + Send;
}
