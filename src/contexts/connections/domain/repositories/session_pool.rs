use std::future::Future;

use crate::contexts::connections::domain::errors::ConnectionError;
use crate::contexts::connections::domain::types::{ExecutionLimits, QueryOutput};

pub trait SessionPool: Send + Sync {
    fn run_read_only(
        &self,
        connection: &str,
        database: Option<&str>,
        sql: &str,
        limits: ExecutionLimits,
    ) -> impl Future<Output = Result<QueryOutput, ConnectionError>> + Send;

    fn run_cached_read_only(
        &self,
        connection: &str,
        database: Option<&str>,
        sql: &str,
        limits: ExecutionLimits,
    ) -> impl Future<Output = Result<QueryOutput, ConnectionError>> + Send;
}
