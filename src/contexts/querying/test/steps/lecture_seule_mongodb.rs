use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use scarif_database_mcp::contexts::querying::domain::repositories::PseudonymTranslator;
use scarif_database_mcp::contexts::querying::domain::services::mongo_guard::{
    InspectedMongo, inspect_aggregate, inspect_find,
};
use scarif_database_mcp::contexts::querying::domain::types::{
    AggregateRequest, FindRequest, MongoCommand, MongoReadPreference, QueryLimits, Translations,
};
use serde_json::{Value, json};

use crate::querying_world::{MongoAnswer, PseudonymBook, QueryingWorld};

fn docstring_json(step: &Step) -> Value {
    let text = step.docstring.as_deref().expect("a docstring");
    serde_json::from_str(text).expect("a JSON docstring")
}

fn guard(
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
    world.mongo = Some(MongoAnswer {
        outcome,
        lookups: store.lookups(),
        clamps,
    });
}

fn refusal(code: String, message: String, hint: Option<String>) -> String {
    format!("{code} {message} {hint:?}")
}

fn find_request(filter: Value) -> FindRequest {
    FindRequest {
        connection: "MONGO".into(),
        database: Some("shop".into()),
        collection: "customers".into(),
        filter: Some(filter),
        ..FindRequest::default()
    }
}

fn aggregate_request(pipeline: Value) -> AggregateRequest {
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
        inspect_find(&request).map_err(|error| refusal(error.code, error.message, error.hint));
    guard(world, inspected, requested);
}

pub fn run_aggregate(world: &mut QueryingWorld, request: AggregateRequest) {
    let requested = (request.max_rows, request.max_bytes, request.timeout_ms);
    let inspected =
        inspect_aggregate(&request).map_err(|error| refusal(error.code, error.message, error.hint));
    guard(world, inspected, requested);
}

fn match_stages(count: usize) -> Value {
    Value::Array((0..count).map(|_| json!({"$match": {}})).collect())
}

#[given(regex = r"^l'agent connecté à une base MongoDB$")]
async fn agent_connected_to_mongodb(world: &mut QueryingWorld) {
    world.pseudonyms.clear();
    world.mongo = None;
}

#[when(regex = r"^l'agent lance l'agrégation suivante :$")]
async fn agent_runs_aggregate(world: &mut QueryingWorld, step: &Step) {
    run_aggregate(world, aggregate_request(docstring_json(step)));
}

#[when(regex = r"^l'agent cherche avec le filtre suivant :$")]
async fn agent_runs_find(world: &mut QueryingWorld, step: &Step) {
    run_find(world, find_request(docstring_json(step)));
}

#[when(regex = r#"^l'agent cherche dans la base "([^"]*)"$"#)]
async fn agent_finds_in_database(world: &mut QueryingWorld, database: String) {
    run_find(
        world,
        FindRequest {
            database: Some(database),
            ..find_request(json!({}))
        },
    );
}

#[when(regex = r#"^l'agent cherche dans la collection "([^"]*)"$"#)]
async fn agent_finds_in_collection(world: &mut QueryingWorld, collection: String) {
    run_find(
        world,
        FindRequest {
            collection,
            ..find_request(json!({}))
        },
    );
}

#[when(regex = r"^l'agent lance une agrégation de (\d+) étapes \$match$")]
async fn agent_runs_long_pipeline(world: &mut QueryingWorld, count: usize) {
    run_aggregate(world, aggregate_request(match_stages(count)));
}

#[when(regex = r"^l'agent lance un \$facet de (\d+) branches de (\d+) étapes \$match$")]
async fn agent_runs_wide_facet(world: &mut QueryingWorld, branches: usize, count: usize) {
    let facet: serde_json::Map<String, Value> = (0..branches)
        .map(|branch| (format!("b{branch}"), match_stages(count)))
        .collect();
    run_aggregate(world, aggregate_request(json!([{ "$facet": facet }])));
}

#[when(regex = r"^l'agent cherche avec un filtre imbriqué sur (\d+) niveaux$")]
async fn agent_runs_deep_filter(world: &mut QueryingWorld, levels: usize) {
    let filter = (1..levels).fold(json!({"a": 1}), |inner, _| json!({ "a": inner }));
    run_find(world, find_request(filter));
}

#[when(regex = r"^l'agent cherche avec un \$in de (\d+) valeurs$")]
async fn agent_runs_long_array(world: &mut QueryingWorld, count: u64) {
    let values: Vec<Value> = (0..count).map(Value::from).collect();
    run_find(world, find_request(json!({"n": {"$in": values}})));
}

#[when(regex = r"^l'agent cherche une valeur texte de (\d+) Ko$")]
async fn agent_runs_large_request(world: &mut QueryingWorld, kilobytes: usize) {
    run_find(
        world,
        find_request(json!({"note": "x".repeat(kilobytes * 1024)})),
    );
}

#[when(regex = r"^l'agent cherche avec une limite de (\d+) documents$")]
async fn agent_finds_with_limit(world: &mut QueryingWorld, limit: u64) {
    run_find(
        world,
        FindRequest {
            limit: Some(limit),
            ..find_request(json!({}))
        },
    );
}

#[when(regex = r"^l'agent lance une agrégation avec un délai de (\d+) ms$")]
async fn agent_aggregates_with_timeout(world: &mut QueryingWorld, timeout: u64) {
    run_aggregate(
        world,
        AggregateRequest {
            timeout_ms: Some(timeout),
            ..aggregate_request(json!([{"$count": "n"}]))
        },
    );
}

#[then(regex = r"^la demande est acceptée$")]
async fn request_accepted(world: &mut QueryingWorld) {
    world.accepted_translations();
}

#[then(regex = r#"^la demande est refusée par la règle "([^"]+)"$"#)]
async fn request_refused_by(world: &mut QueryingWorld, rule: String) {
    let text = world.refusal_text();
    assert!(text.starts_with(&format!("{rule} ")), "{text}");
}

#[then(regex = r#"^le refus nomme "([^"]+)"$"#)]
async fn refusal_names(world: &mut QueryingWorld, name: String) {
    let text = world.refusal_text();
    assert!(text.contains(&name), "{text}");
}

#[then(regex = r"^rien n'a été demandé au magasin de pseudonymes$")]
async fn store_untouched(world: &mut QueryingWorld) {
    assert_eq!(world.mongo_answer().lookups, 0);
}

#[then(regex = r"^le pipeline envoyé se termine par une limite de (\d+) documents$")]
async fn pipeline_ends_with_limit(world: &mut QueryingWorld, limit: u64) {
    let guarded = world.mongo_answer().outcome.clone().expect("accepted");
    let MongoCommand::Aggregate(command) = guarded.command else {
        panic!("an aggregate command");
    };
    assert_eq!(command.pipeline.last(), Some(&json!({ "$limit": limit })));
}

#[then(regex = r"^la recherche envoyée lit au plus (\d+) documents$")]
async fn find_reads_at_most(world: &mut QueryingWorld, limit: u64) {
    let guarded = world.mongo_answer().outcome.clone().expect("accepted");
    let MongoCommand::Find(command) = guarded.command else {
        panic!("a find command");
    };
    assert_eq!(command.limit, limit);
}

#[then(
    regex = r"^l'exécution est bornée à (\d+) ms, sans écriture sur disque, sur un secondaire de préférence$"
)]
async fn execution_is_bounded(world: &mut QueryingWorld, timeout: u64) {
    let guarded = world.mongo_answer().outcome.clone().expect("accepted");
    assert_eq!(guarded.execution.max_time_ms, timeout);
    assert!(!guarded.execution.allow_disk_use);
    assert_eq!(
        guarded.execution.read_preference,
        MongoReadPreference::SecondaryPreferred
    );
}

#[then(regex = r"^la demande n'est pas ramenée$")]
async fn request_not_clamped(world: &mut QueryingWorld) {
    let answer = world.mongo_answer();
    let guarded = answer.outcome.clone().expect("accepted");
    assert!(answer.clamps.is_empty(), "{:?}", answer.clamps);
    assert!(guarded.clamps.is_empty(), "{:?}", guarded.clamps);
}
