use cucumber::{given, then, when};
use serde_json::json;

use crate::querying_world::QueryingWorld;

#[given(regex = r"^l'agent connecté à la base de test$")]
async fn agent_connected(world: &mut QueryingWorld) {
    world.connect_to_test_database();
}

#[when(regex = r#"^l'agent exécute la requête "(.+)"$"#)]
async fn agent_runs_query(world: &mut QueryingWorld, sql: String) {
    world.ask(&sql).await;
}

#[then(regex = r"^la requête réussit$")]
async fn query_succeeds(world: &mut QueryingWorld) {
    let answer = world.answer();
    assert!(!answer.refused, "{}", answer.text);
}

#[then(regex = r"^la réponse compte (\d+) lignes$")]
async fn answer_has_rows(world: &mut QueryingWorld, expected: usize) {
    assert_eq!(
        world.answer().rows().len(),
        expected,
        "{}",
        world.answer().text
    );
}

#[then(regex = r"^la requête est refusée parce qu'elle n'est pas en lecture seule$")]
async fn query_is_refused_as_read_only(world: &mut QueryingWorld) {
    let answer = world.answer();
    assert!(answer.refused, "{}", answer.text);
    assert_eq!(
        answer.body()["error"]["category"],
        "refused_read_only",
        "{}",
        answer.text
    );
}

#[then(regex = r"^la première cellule de la réponse vaut (\d+)$")]
async fn first_cell_equals(world: &mut QueryingWorld, expected: u64) {
    assert_eq!(world.answer().rows()[0][0], json!(expected));
}
