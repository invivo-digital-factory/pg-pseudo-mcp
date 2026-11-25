use cucumber::{then, when};
use scarif_database_mcp::contexts::querying::presentation::QueryArgs;
use serde_json::Value;

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
