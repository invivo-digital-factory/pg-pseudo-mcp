use cucumber::{given, then, when};
use scarif_database_mcp::contexts::querying::domain::types::FindRequest;
use serde_json::{Value, json};

use crate::querying_steps::support::{aggregate_request, find_request, run_aggregate, run_find};
use crate::querying_world::{CANARY_VALUES, MongoUser, QueryingWorld};

#[then(regex = r#"^le refus renvoie vers "([^"]+)"$"#)]
async fn refusal_points_to(world: &mut QueryingWorld, tool: String) {
    let answer = world.answer();
    let hint = answer.body()["error"]["hint"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(hint.contains(&tool), "{}", answer.text);
}

#[then(regex = r#"^le refus ne contient pas "([^"]+)"$"#)]
async fn refusal_omits(world: &mut QueryingWorld, text: String) {
    let refusal = world.refusal_text();
    assert!(!refusal.contains(&text), "{refusal}");
}

#[then(regex = r"^le refus ne contient aucune valeur canari$")]
async fn refusal_has_no_canary(world: &mut QueryingWorld) {
    let text = world.refusal_text().to_lowercase();
    for value in CANARY_VALUES {
        assert!(!text.contains(&value.to_lowercase()), "{value} in {text}");
    }
    assert!(!text.contains("canary-mail"), "{text}");
}

fn match_stages(count: usize) -> Value {
    Value::Array((0..count).map(|_| json!({"$match": {}})).collect())
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

#[given(regex = r"^la connexion MongoDB de test avec un utilisateur autorisé à écrire$")]
async fn mongo_writer_connection(world: &mut QueryingWorld) {
    world.connect_to_test_mongodb_as(&MongoUser {
        name: "atlas-writer",
        password: "atlas-writer-test-pw",
        default_database: true,
    });
}
