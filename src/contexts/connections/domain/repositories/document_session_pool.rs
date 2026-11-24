use std::future::Future;

use crate::contexts::connections::domain::errors::ConnectionError;
use crate::contexts::connections::domain::types::{
    AggregateRequest, CollectionDescription, CollectionInfo, DatabaseNames, DocumentOutput,
    ExecutionLimits, FindRequest, SampledField,
};

pub trait DocumentSessionPool: Send + Sync {
    fn find(
        &self,
        connection: &str,
        request: &FindRequest,
        limits: ExecutionLimits,
    ) -> impl Future<Output = Result<DocumentOutput, ConnectionError>> + Send;

    fn aggregate(
        &self,
        connection: &str,
        request: &AggregateRequest,
        limits: ExecutionLimits,
    ) -> impl Future<Output = Result<DocumentOutput, ConnectionError>> + Send;

    fn database_names(
        &self,
        connection: &str,
    ) -> impl Future<Output = Result<DatabaseNames, ConnectionError>> + Send;

    fn collections(
        &self,
        connection: &str,
        database: Option<&str>,
    ) -> impl Future<Output = Result<Vec<CollectionInfo>, ConnectionError>> + Send;

    fn describe_collection(
        &self,
        connection: &str,
        database: Option<&str>,
        collection: &str,
    ) -> impl Future<Output = Result<CollectionDescription, ConnectionError>> + Send;

    fn sample_fields(
        &self,
        connection: &str,
        database: Option<&str>,
        collection: &str,
    ) -> impl Future<Output = Result<Vec<SampledField>, ConnectionError>> + Send;
}
