use std::future::Future;

use crate::contexts::querying::domain::errors::DatabaseFailure;
use crate::contexts::querying::domain::types::{ExecutionLimits, QueryOutput};

pub trait ReadOnlyDatabase: Send + Sync {
    fn run(
        &self,
        connection: &str,
        sql: &str,
        limits: ExecutionLimits,
    ) -> impl Future<Output = Result<QueryOutput, DatabaseFailure>> + Send;

    fn explain(
        &self,
        connection: &str,
        explain_sql: &str,
        limits: ExecutionLimits,
    ) -> impl Future<Output = Result<QueryOutput, DatabaseFailure>> + Send;

    fn read_catalog(
        &self,
        connection: &str,
        catalog_sql: &str,
        limits: ExecutionLimits,
    ) -> impl Future<Output = Result<QueryOutput, DatabaseFailure>> + Send;
}
