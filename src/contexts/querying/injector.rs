use std::sync::Arc;

use crate::contexts::querying::domain::repositories::{
    AnonymizationPorts, ColumnPolicySource, ReadOnlyDatabase,
};
use crate::contexts::querying::use_cases::{
    DescribeTable, ExplainQuery, ListDatabases, ListTables, ReadColumnPolicy, RunQuery,
    SearchSchema,
};

pub struct QueryingContext<D: ReadOnlyDatabase> {
    pub run_query: RunQuery<D>,
    pub explain_query: ExplainQuery<D>,
    pub search_schema: SearchSchema<D>,
    pub list_databases: ListDatabases<D>,
    pub list_tables: ListTables<D>,
    pub describe_table: DescribeTable<D>,
    pub read_column_policy: ReadColumnPolicy,
}

impl<D: ReadOnlyDatabase> QueryingContext<D> {
    pub fn new(
        database: Arc<D>,
        anonymization: Result<AnonymizationPorts, String>,
        policy: Arc<dyn ColumnPolicySource>,
    ) -> Self {
        Self {
            run_query: RunQuery::new(Arc::clone(&database), anonymization.clone())
                .with_column_policy(Arc::clone(&policy)),
            explain_query: ExplainQuery::new(Arc::clone(&database), anonymization.clone()),
            search_schema: SearchSchema::new(Arc::clone(&database), anonymization.clone()),
            list_databases: ListDatabases::new(Arc::clone(&database), anonymization.clone()),
            list_tables: ListTables::new(Arc::clone(&database), anonymization.clone()),
            describe_table: DescribeTable::new(database, anonymization)
                .with_column_policy(Arc::clone(&policy)),
            read_column_policy: ReadColumnPolicy::new(policy),
        }
    }
}
