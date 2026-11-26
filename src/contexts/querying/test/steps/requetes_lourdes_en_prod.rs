use cucumber::{given, then, when};
use scarif_database_mcp::contexts::querying::presentation::{FindArgs, QueryArgs};
use serde_json::Value;

use crate::bdd_support::CONNECTION;
use crate::querying_steps::support::find;
use crate::querying_world::QueryingWorld;

#[given(regex = r"^l'agent connecté à la base de test déclarée prod avec « (.+) »$")]
async fn connected_on_prod(world: &mut QueryingWorld, extra: String) {
    world.connect_to_test_database_on_prod(&extra);
}

#[given(regex = r"^la connexion MongoDB de test déclarée prod avec « (.+) »$")]
async fn mongodb_on_prod(world: &mut QueryingWorld, extra: String) {
    world.connect_to_test_mongodb_on_prod(&extra);
}

#[when(regex = r#"^l'agent exécute avec confirm_heavy la requête "(.+)"$"#)]
async fn runs_confirmed(world: &mut QueryingWorld, sql: String) {
    world
        .ask_with(QueryArgs {
            connection: CONNECTION.to_string(),
            sql,
            confirm_heavy: Some(true),
            ..QueryArgs::default()
        })
        .await;
}

fn find_args(collection: String, filter: &str, confirm_heavy: bool) -> FindArgs {
    FindArgs {
        collection,
        filter: Some(serde_json::from_str(filter).expect("a JSON filter")),
        confirm_heavy: confirm_heavy.then_some(true),
        ..FindArgs::default()
    }
}

#[when(regex = r#"^l'agent cherche dans "([^"]+)" avec le filtre (\{.*\})$"#)]
async fn finds_with_filter(world: &mut QueryingWorld, collection: String, filter: String) {
    find(world, find_args(collection, &filter, false)).await;
}

#[when(regex = r#"^l'agent cherche avec confirm_heavy dans "([^"]+)" avec le filtre (\{.*\})$"#)]
async fn finds_confirmed(world: &mut QueryingWorld, collection: String, filter: String) {
    find(world, find_args(collection, &filter, true)).await;
}

fn estimate(world: &QueryingWorld) -> Value {
    let answer = world.answer();
    assert!(answer.refused, "{}", answer.text);
    answer.body()["error"]["estimate"].clone()
}

#[then(regex = r"^l'estimation donne un coût au-dessus de la limite de (\d+)$")]
async fn cost_above(world: &mut QueryingWorld, limit: u64) {
    let estimate = estimate(world);
    assert_eq!(estimate["cost_limit"], limit, "{estimate}");
    let cost = estimate["total_cost"].as_f64().expect("a total cost");
    assert!(cost > limit as f64, "{estimate}");
}

#[then(regex = r#"^l'estimation nomme le parcours "([^"]+)" de "([^"]+)"$"#)]
async fn names_full_scan(world: &mut QueryingWorld, node: String, relation: String) {
    let estimate = estimate(world);
    let scans = estimate["full_scans"].as_array().expect("full scans");
    assert!(
        scans
            .iter()
            .any(|scan| scan["node"] == node.as_str() && scan["relation"] == relation.as_str()),
        "{estimate}"
    );
}
