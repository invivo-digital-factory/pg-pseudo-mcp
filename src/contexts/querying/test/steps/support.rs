use scarif_database_mcp::composition::tools;
use scarif_database_mcp::contexts::querying::domain::repositories::PseudonymTranslator;
use scarif_database_mcp::contexts::querying::domain::services::mongo_guard::{
    InspectedMongo, inspect_aggregate, inspect_find,
};
use scarif_database_mcp::contexts::querying::domain::types::{
    AggregateRequest, ColumnSource, FieldSource, FindRequest, QueryLimits, Translations,
};
use scarif_database_mcp::contexts::querying::presentation::{
    AggregateArgs, ExplainArgs, FindArgs, ListTablesArgs,
};
use serde_json::{Value, json};

use crate::bdd_support::CONNECTION;
use crate::querying_world::{
    MONGO_CONNECTION, MongoAnswer, PseudonymBook, QueryingWorld, refusal_text,
};

pub fn checked(world: &QueryingWorld) -> Value {
    world.answer().checked_body()
}

pub async fn list_tables(world: &mut QueryingWorld, args: ListTablesArgs) {
    let result = tools::list_tables(
        world.contexts(),
        ListTablesArgs {
            connection: CONNECTION.into(),
            schema: Some("public".into()),
            ..args
        },
    )
    .await;
    world.record(&result);
}

pub fn error(world: &QueryingWorld) -> Value {
    let answer = world.answer();
    assert!(answer.refused, "{}", answer.text);
    answer.body()["error"].clone()
}

pub fn guard(
    world: &mut QueryingWorld,
    inspected: Result<InspectedMongo, String>,
    requested: (Option<u64>, Option<u64>, Option<u64>),
) {
    let store = PseudonymBook::new(&world.pseudonyms);
    let (limits, clamps) = QueryLimits::from_request(requested.0, requested.1, requested.2);
    let outcome = inspected.map(|inspected| {
        let mut translations = Translations::default();
        for literal in inspected.literals() {
            translations.record(&literal, store.lookup(&literal));
        }
        inspected.finish(&translations, &limits)
    });
    world.record_mongo_guard(MongoAnswer {
        outcome,
        lookups: store.lookups(),
        clamps,
    });
}

pub fn find_request(filter: Value) -> FindRequest {
    FindRequest {
        connection: "MONGO".into(),
        database: Some("shop".into()),
        collection: "customers".into(),
        filter: Some(filter),
        ..FindRequest::default()
    }
}

pub fn aggregate_request(pipeline: Value) -> AggregateRequest {
    AggregateRequest {
        connection: "MONGO".into(),
        database: Some("shop".into()),
        collection: "orders".into(),
        pipeline: pipeline.as_array().expect("a pipeline array").clone(),
        ..AggregateRequest::default()
    }
}

pub fn run_find(world: &mut QueryingWorld, request: FindRequest) {
    let requested = (request.max_rows, request.max_bytes, request.timeout_ms);
    let inspected =
        inspect_find(&request).map_err(|error| refusal_text(error.code, error.message, error.hint));
    guard(world, inspected, requested);
}

pub fn run_aggregate(world: &mut QueryingWorld, request: AggregateRequest) {
    let requested = (request.max_rows, request.max_bytes, request.timeout_ms);
    let inspected = inspect_aggregate(&request)
        .map_err(|error| refusal_text(error.code, error.message, error.hint));
    guard(world, inspected, requested);
}

pub async fn find(world: &mut QueryingWorld, args: FindArgs) {
    let result = tools::find(
        world.contexts(),
        FindArgs {
            connection: MONGO_CONNECTION.into(),
            ..args
        },
    )
    .await;
    world.record(&result);
}

pub async fn aggregate(world: &mut QueryingWorld, args: AggregateArgs) {
    world.pipeline = Some(args.pipeline.clone());
    let result = tools::aggregate(
        world.contexts(),
        AggregateArgs {
            connection: MONGO_CONNECTION.into(),
            ..args
        },
    )
    .await;
    world.record(&result);
}

pub fn by_id(collection: &str, id: &str) -> FindArgs {
    FindArgs {
        collection: collection.into(),
        filter: Some(json!({"_id": {"$oid": id}})),
        ..FindArgs::default()
    }
}

pub async fn explain(world: &mut QueryingWorld, sql: String) {
    let result = tools::explain(
        world.contexts(),
        ExplainArgs {
            connection: CONNECTION.into(),
            sql,
            database: None,
        },
    )
    .await;
    world.record(&result);
}

pub fn field(collection: &str, path: &str) -> FieldSource {
    let (database, collection) = collection
        .split_once('.')
        .expect("database.collection in the scenario");
    FieldSource {
        database: database.to_string(),
        collection: collection.to_string(),
        path: path.to_string(),
    }
}

pub fn column(qualified: &str) -> ColumnSource {
    let parts: Vec<&str> = qualified.split('.').collect();
    ColumnSource {
        schema: parts[0].to_string(),
        table: parts[1].to_string(),
        column: parts[2].to_string(),
    }
}
