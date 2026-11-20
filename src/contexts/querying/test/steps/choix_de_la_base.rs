use cucumber::{then, when};
use serde_json::{Value, json};

use crate::bdd_support::database_target;
use crate::querying_world::QueryingWorld;

fn listed_names(world: &QueryingWorld) -> Vec<String> {
    let answer = world.answer();
    assert!(!answer.refused, "{}", answer.text);
    answer.body()["databases"]
        .as_array()
        .expect("databases array")
        .iter()
        .map(|name| name.as_str().expect("database name").to_string())
        .collect()
}

#[when(regex = r"^l'agent liste les bases de la connexion$")]
async fn agent_lists_databases(world: &mut QueryingWorld) {
    world.list_databases().await;
}

#[when(regex = r#"^l'agent interroge la base "(.+)" avec la requête "(.+)"$"#)]
async fn agent_queries_database(world: &mut QueryingWorld, database: String, sql: String) {
    world.ask_in(&sql, Some(&database)).await;
}

#[then(regex = r#"^la liste contient la base "(.+)"$"#)]
async fn list_contains(world: &mut QueryingWorld, name: String) {
    assert!(
        listed_names(world).contains(&name),
        "{}",
        world.answer().text
    );
}

#[then(regex = r"^la liste contient la base de test$")]
async fn list_contains_test_database(world: &mut QueryingWorld) {
    assert!(
        listed_names(world).contains(&database_target().database),
        "{}",
        world.answer().text
    );
}

#[then(regex = r"^la liste ne contient aucun modèle de base$")]
async fn list_has_no_template(world: &mut QueryingWorld) {
    assert!(
        listed_names(world)
            .iter()
            .all(|name| !name.starts_with("template")),
        "{}",
        world.answer().text
    );
}

#[then(regex = r"^la base de la connexion est celle de test$")]
async fn default_is_test_database(world: &mut QueryingWorld) {
    assert_eq!(
        world.answer().body()["default"],
        Value::String(database_target().database)
    );
}

#[then(regex = r"^la première cellule de la réponse est vraie$")]
async fn first_cell_is_true(world: &mut QueryingWorld) {
    assert_eq!(world.answer().rows()[0][0], json!(true));
}

#[then(regex = r"^la première cellule de la réponse est fausse$")]
async fn first_cell_is_false(world: &mut QueryingWorld) {
    assert_eq!(world.answer().rows()[0][0], json!(false));
}
