use cucumber::{then, when};
use scarif_database_mcp::contexts::querying::domain::types::{
    AggregateRequest, FindRequest, MongoCommand, MongoReadPreference,
};
use scarif_database_mcp::contexts::querying::presentation::{AggregateArgs, FindArgs, QueryArgs};
use serde_json::{Value, json};

use crate::querying_steps::support::{
    aggregate, aggregate_request, checked, find, find_request, run_aggregate, run_find,
};
use crate::querying_world::QueryingWorld;

fn truncations(world: &QueryingWorld) -> Vec<Value> {
    let answer = world.answer();
    assert!(!answer.refused, "{}", answer.text);
    answer.body()["truncated"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn truncation(world: &QueryingWorld, by: &str, limit: u64) -> Value {
    truncations(world)
        .into_iter()
        .find(|item| item["by"] == by && item["limit"] == limit)
        .unwrap_or_else(|| panic!("no {by} truncation at {limit} in {}", world.answer().text))
}

fn first_cell(world: &QueryingWorld) -> String {
    let answer = world.answer();
    assert!(!answer.refused, "{}", answer.text);
    answer.rows()[0][0]
        .as_str()
        .expect("a text cell")
        .to_string()
}

#[when(
    regex = r#"^l'agent exécute la requête "(.+)" avec (max_rows|max_bytes|timeout_ms|cell_offset) à (\d+)$"#
)]
async fn agent_runs_query_with(
    world: &mut QueryingWorld,
    sql: String,
    parameter: String,
    value: u64,
) {
    let mut args = QueryArgs {
        sql,
        ..QueryArgs::default()
    };
    match parameter.as_str() {
        "max_rows" => args.max_rows = Some(value),
        "max_bytes" => args.max_bytes = Some(value),
        "timeout_ms" => args.timeout_ms = Some(value),
        _ => args.cell_offset = Some(value),
    }
    world.ask_with(args).await;
}

#[when(regex = r#"^l'agent exécute la requête "(.+)" avec max_rows à (\d+) et max_bytes à (\d+)$"#)]
async fn agent_runs_query_with_rows_and_bytes(
    world: &mut QueryingWorld,
    sql: String,
    rows: u64,
    bytes: u64,
) {
    world
        .ask_with(QueryArgs {
            sql,
            max_rows: Some(rows),
            max_bytes: Some(bytes),
            ..QueryArgs::default()
        })
        .await;
}

#[then(regex = r#"^l'avertissement "([^"]+)" annonce (\d+) ramené à (\d+)$"#)]
async fn warning_announces_clamp(
    world: &mut QueryingWorld,
    code: String,
    requested: u64,
    applied: u64,
) {
    if let Some(mongo) = &world.mongo {
        let guarded = mongo.outcome.clone().expect("accepted");
        let found = mongo.clamps.iter().chain(&guarded.clamps).any(|clamp| {
            clamp.code == code && clamp.requested == requested && clamp.applied == applied
        });
        assert!(found, "{:?} {:?}", mongo.clamps, guarded.clamps);
        return;
    }
    let answer = world.answer();
    let message = format!("requested {requested} was capped to {applied}");
    let found = answer.body()["warnings"]
        .as_array()
        .is_some_and(|warnings| {
            warnings
                .iter()
                .any(|warning| warning["code"] == code.as_str() && warning["message"] == message)
        });
    assert!(found, "{}", answer.text);
}

#[then(regex = r#"^la réponse est coupée par "([^"]+)" à (\d+)$"#)]
async fn answer_cut_by(world: &mut QueryingWorld, by: String, limit: u64) {
    truncation(world, &by, limit);
}

#[then(
    regex = r#"^la réponse est coupée par "([^"]+)" à (\d+) sur (?:la colonne|le champ) "([^"]+)"$"#
)]
async fn answer_cut_on(world: &mut QueryingWorld, by: String, limit: u64, name: String) {
    let cut = truncation(world, &by, limit);
    let named = cut["column"] == name.as_str() || cut["field"] == name.as_str();
    assert!(named, "{cut}");
}

#[then(regex = r#"^l'indice de la coupure contient "([^"]+)"$"#)]
async fn truncation_hint_contains(world: &mut QueryingWorld, text: String) {
    let found = truncations(world)
        .iter()
        .any(|item| item["hint"].as_str().unwrap_or_default().contains(&text));
    assert!(found, "{}", world.answer().text);
}

#[then(regex = r#"^la première cellule compte (\d+) caractères suivis de "([^"]+)"$"#)]
async fn first_cell_is_cut(world: &mut QueryingWorld, characters: usize, marker: String) {
    let cell = first_cell(world);
    let kept = cell
        .strip_suffix(marker.as_str())
        .unwrap_or_else(|| panic!("{cell} does not end with {marker}"));
    assert_eq!(kept.chars().count(), characters, "{cell}");
}

#[then(regex = r#"^la première cellule commence par "([^"]+)"$"#)]
async fn first_cell_starts_with(world: &mut QueryingWorld, start: String) {
    let cell = first_cell(world);
    assert!(cell.starts_with(&start), "{cell}");
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

#[when(regex = r#"^l'agent cherche dans "([^"]+)" tous les documents avec max_bytes à (\d+)$"#)]
async fn find_all_within_bytes(world: &mut QueryingWorld, collection: String, bytes: u64) {
    find(
        world,
        FindArgs {
            collection,
            max_bytes: Some(bytes),
            ..FindArgs::default()
        },
    )
    .await;
}

#[when(
    regex = r#"^l'agent lit dans "([^"]+)" un texte de (\d+) caractères et un tableau de (\d+) éléments$"#
)]
async fn aggregate_long_values(
    world: &mut QueryingWorld,
    collection: String,
    characters: usize,
    items: u64,
) {
    let text = "abcdefghij".repeat(characters / 10);
    let pipeline = vec![
        json!({"$match": {"_id": {"$oid": "65a000000000000000000001"}}}),
        json!({"$project": {"_id": 0, "note": {"$literal": text}, "items": {"$range": [0, items]}}}),
    ];
    aggregate(
        world,
        AggregateArgs {
            collection,
            pipeline,
            ..AggregateArgs::default()
        },
    )
    .await;
}

#[when(regex = r"^l'agent relit le même texte avec cell_offset à (\d+)$")]
async fn aggregate_again_from_offset(world: &mut QueryingWorld, offset: u64) {
    let pipeline = world.pipeline.clone().expect("an aggregation was run");
    aggregate(
        world,
        AggregateArgs {
            collection: "customers".into(),
            pipeline,
            cell_offset: Some(offset),
            ..AggregateArgs::default()
        },
    )
    .await;
}

fn first_document_field(world: &QueryingWorld, field: &str) -> Value {
    let body = checked(world);
    body["documents"][0][field].clone()
}

#[then(
    regex = r#"^le champ "([^"]+)" du premier document compte (\d+) caractères suivis de "([^"]+)"$"#
)]
async fn first_document_text_cut(
    world: &mut QueryingWorld,
    field: String,
    characters: usize,
    marker: String,
) {
    let value = first_document_field(world, &field);
    let text = value.as_str().expect("a text value");
    let kept = text
        .strip_suffix(marker.as_str())
        .unwrap_or_else(|| panic!("{text} does not end with {marker}"));
    assert_eq!(kept.chars().count(), characters, "{text}");
}

#[then(regex = r#"^le champ "([^"]+)" du premier document commence par "([^"]+)"$"#)]
async fn first_document_text_starts(world: &mut QueryingWorld, field: String, start: String) {
    let value = first_document_field(world, &field);
    let text = value.as_str().expect("a text value");
    assert!(text.starts_with(&start), "{text}");
}

#[then(regex = r#"^le champ "([^"]+)" du premier document compte (\d+) éléments$"#)]
async fn first_document_array_len(world: &mut QueryingWorld, field: String, count: usize) {
    let value = first_document_field(world, &field);
    assert_eq!(value.as_array().expect("an array").len(), count, "{value}");
}
