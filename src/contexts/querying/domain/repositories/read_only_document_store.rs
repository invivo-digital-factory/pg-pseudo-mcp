use std::future::Future;

use crate::contexts::querying::domain::errors::DatabaseFailure;
use crate::contexts::querying::domain::types::{
    CollectionDetails, CollectionEntry, DocumentOutput, ExecutionLimits, FieldSample,
    GuardedAggregate, GuardedFind,
};

pub trait ReadOnlyDocumentStore: Send + Sync {
    fn default_database(
        &self,
        connection: &str,
    ) -> impl Future<Output = Result<Option<String>, DatabaseFailure>> + Send;

    fn find(
        &self,
        connection: &str,
        database: &str,
        collection: &str,
        find: &GuardedFind,
        limits: ExecutionLimits,
    ) -> impl Future<Output = Result<DocumentOutput, DatabaseFailure>> + Send;

    fn aggregate(
        &self,
        connection: &str,
        database: &str,
        collection: &str,
        aggregate: &GuardedAggregate,
        limits: ExecutionLimits,
    ) -> impl Future<Output = Result<DocumentOutput, DatabaseFailure>> + Send;

    fn databases(
        &self,
        connection: &str,
    ) -> impl Future<Output = Result<Vec<String>, DatabaseFailure>> + Send;

    fn collections(
        &self,
        connection: &str,
        database: &str,
    ) -> impl Future<Output = Result<Vec<CollectionEntry>, DatabaseFailure>> + Send;

    fn describe_collection(
        &self,
        connection: &str,
        database: &str,
        collection: &str,
    ) -> impl Future<Output = Result<CollectionDetails, DatabaseFailure>> + Send;

    fn sample_fields(
        &self,
        connection: &str,
        database: &str,
        collection: &str,
    ) -> impl Future<Output = Result<Vec<FieldSample>, DatabaseFailure>> + Send;
}
