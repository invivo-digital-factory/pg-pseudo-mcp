use std::sync::Arc;

use crate::contexts::querying::domain::repositories::{
    AnonymizationPorts, ColumnPolicySource, ConnectionEngines, DocumentPorts, ReadOnlyDatabase,
    ReadOnlyDocumentStore,
};
use crate::contexts::querying::domain::types::Engine;
use crate::contexts::querying::use_cases::{
    DescribeCollection, DescribeTable, ExplainQuery, ListCollections, ListDatabases,
    ListMongoDatabases, ListTables, ReadColumnPolicy, RunAggregate, RunFind, RunQuery,
    SearchCollections, SearchSchema,
};

pub struct QueryingContext<D: ReadOnlyDatabase, S: ReadOnlyDocumentStore, E: ConnectionEngines> {
    pub run_query: RunQuery<D>,
    pub explain_query: ExplainQuery<D>,
    pub search_schema: SearchSchema<D>,
    pub list_databases: ListDatabases<D>,
    pub list_tables: ListTables<D>,
    pub describe_table: DescribeTable<D>,
    pub read_column_policy: ReadColumnPolicy,
    pub documents: DocumentUseCases<S>,
    engines: Arc<E>,
}

pub struct DocumentUseCases<S: ReadOnlyDocumentStore> {
    pub run_find: RunFind<S>,
    pub run_aggregate: RunAggregate<S>,
    pub list_databases: ListMongoDatabases<S>,
    pub list_collections: ListCollections<S>,
    pub describe_collection: DescribeCollection<S>,
    pub search_collections: SearchCollections<S>,
}

pub struct Stores<D, S, E> {
    pub database: Arc<D>,
    pub documents: Arc<S>,
    pub engines: Arc<E>,
}

pub struct Ports {
    pub rows: Result<AnonymizationPorts, String>,
    pub documents: Result<DocumentPorts, String>,
}

impl<D: ReadOnlyDatabase, S: ReadOnlyDocumentStore, E: ConnectionEngines> QueryingContext<D, S, E> {
    pub fn new(stores: Stores<D, S, E>, ports: Ports, policy: Arc<dyn ColumnPolicySource>) -> Self {
        let Stores {
            database,
            documents,
            engines,
        } = stores;
        let anonymization = ports.rows;
        let document_ports = ports.documents;
        Self {
            run_query: RunQuery::new(Arc::clone(&database), anonymization.clone())
                .with_column_policy(Arc::clone(&policy)),
            explain_query: ExplainQuery::new(Arc::clone(&database), anonymization.clone()),
            search_schema: SearchSchema::new(Arc::clone(&database), anonymization.clone()),
            list_databases: ListDatabases::new(Arc::clone(&database), anonymization.clone()),
            list_tables: ListTables::new(Arc::clone(&database), anonymization.clone()),
            describe_table: DescribeTable::new(database, anonymization)
                .with_column_policy(Arc::clone(&policy)),
            documents: DocumentUseCases {
                run_find: RunFind::new(Arc::clone(&documents), document_ports.clone())
                    .with_column_policy(Arc::clone(&policy)),
                run_aggregate: RunAggregate::new(Arc::clone(&documents), document_ports.clone())
                    .with_column_policy(Arc::clone(&policy)),
                list_databases: ListMongoDatabases::new(
                    Arc::clone(&documents),
                    document_ports.clone(),
                ),
                list_collections: ListCollections::new(
                    Arc::clone(&documents),
                    document_ports.clone(),
                ),
                describe_collection: DescribeCollection::new(
                    Arc::clone(&documents),
                    document_ports.clone(),
                )
                .with_column_policy(Arc::clone(&policy)),
                search_collections: SearchCollections::new(documents, document_ports),
            },
            read_column_policy: ReadColumnPolicy::new(policy),
            engines,
        }
    }

    pub async fn engine_of(&self, connection: &str) -> Option<Engine> {
        self.engines.engine_of(connection).await
    }
}
