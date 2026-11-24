use rmcp::model::CallToolResult;

use super::contexts::Contexts;
use crate::contexts::querying::domain::types::Engine;
use crate::contexts::querying::presentation::{
    self as querying, AggregateArgs, DescribeTableArgs, ExplainArgs, FindArgs, ListDatabasesArgs,
    ListTablesArgs, QueryArgs, SearchSchemaArgs,
};

async fn is_mongodb(contexts: &Contexts, connection: &str) -> bool {
    contexts.querying.engine_of(connection).await == Some(Engine::MongoDb)
}

pub async fn query(contexts: &Contexts, args: QueryArgs) -> CallToolResult {
    if is_mongodb(contexts, &args.connection).await {
        return querying::use_document_tools();
    }
    querying::query(&contexts.querying.run_query, args).await
}

pub async fn explain(contexts: &Contexts, args: ExplainArgs) -> CallToolResult {
    if is_mongodb(contexts, &args.connection).await {
        return querying::use_document_tools();
    }
    querying::explain(&contexts.querying.explain_query, args).await
}

pub async fn find(contexts: &Contexts, args: FindArgs) -> CallToolResult {
    if !is_mongodb(contexts, &args.connection).await {
        return querying::use_sql_tools();
    }
    querying::find(&contexts.querying.documents.run_find, args).await
}

pub async fn aggregate(contexts: &Contexts, args: AggregateArgs) -> CallToolResult {
    if !is_mongodb(contexts, &args.connection).await {
        return querying::use_sql_tools();
    }
    querying::aggregate(&contexts.querying.documents.run_aggregate, args).await
}

pub async fn list_databases(contexts: &Contexts, args: ListDatabasesArgs) -> CallToolResult {
    if is_mongodb(contexts, &args.connection).await {
        return querying::list_mongo_databases(&contexts.querying.documents.list_databases, args)
            .await;
    }
    querying::list_databases(&contexts.querying.list_databases, args).await
}

pub async fn list_tables(contexts: &Contexts, args: ListTablesArgs) -> CallToolResult {
    if is_mongodb(contexts, &args.connection).await {
        return querying::list_collections(&contexts.querying.documents.list_collections, args)
            .await;
    }
    querying::list_tables(&contexts.querying.list_tables, args).await
}

pub async fn describe_table(contexts: &Contexts, args: DescribeTableArgs) -> CallToolResult {
    if is_mongodb(contexts, &args.connection).await {
        return querying::describe_collection(
            &contexts.querying.documents.describe_collection,
            args,
        )
        .await;
    }
    querying::describe_table(&contexts.querying.describe_table, args).await
}

pub async fn search_schema(contexts: &Contexts, args: SearchSchemaArgs) -> CallToolResult {
    if is_mongodb(contexts, &args.connection).await {
        return querying::search_collections(&contexts.querying.documents.search_collections, args)
            .await;
    }
    querying::search_schema(&contexts.querying.search_schema, args).await
}
