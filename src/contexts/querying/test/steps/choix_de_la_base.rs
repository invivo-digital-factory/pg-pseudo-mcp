use cucumber::{given, then, when};
use scarif_database_mcp::composition::tools;
use scarif_database_mcp::contexts::querying::presentation::{FindArgs, ListDatabasesArgs};
use serde_json::{Value, json};

use crate::bdd_support::database_target;
use crate::querying_steps::support::{by_id, checked, find};
use crate::querying_world::{MONGO_CONNECTION, MONGO_READER, MongoUser, QueryingWorld};

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

#[given(regex = r"^la connexion MongoDB de test sans base par défaut$")]
async fn mongo_connection_without_database(world: &mut QueryingWorld) {
    world.connect_to_test_mongodb_as(&MongoUser {
        default_database: false,
        ..MONGO_READER
    });
}

#[when(
    regex = r#"^l'agent cherche dans la base "([^"]+)" de la collection "([^"]+)" le document d'identifiant "([0-9a-f]{24})"$"#
)]
async fn find_by_id_in_database(
    world: &mut QueryingWorld,
    database: String,
    collection: String,
    id: String,
) {
    find(
        world,
        FindArgs {
            database: Some(database),
            ..by_id(&collection, &id)
        },
    )
    .await;
}

#[when(regex = r"^l'agent liste les bases de la connexion MongoDB$")]
async fn list_mongo_databases(world: &mut QueryingWorld) {
    let result = tools::list_databases(
        world.contexts(),
        ListDatabasesArgs {
            connection: MONGO_CONNECTION.into(),
        },
    )
    .await;
    world.record(&result);
}

#[then(regex = r#"^la base de la connexion est "([^"]+)"$"#)]
async fn default_database_is(world: &mut QueryingWorld, database: String) {
    let body = checked(world);
    assert_eq!(body["default"], database.as_str(), "{body}");
}

#[then(regex = r#"^la liste ne contient ni "([^"]+)", ni "([^"]+)", ni "([^"]+)"$"#)]
async fn list_excludes(world: &mut QueryingWorld, first: String, second: String, third: String) {
    let body = checked(world);
    let databases = body["databases"].as_array().expect("databases");
    for name in [first, second, third] {
        assert!(!databases.contains(&json!(name)), "{body}");
    }
}

#[then(regex = r#"^la réponse contient "([^"]+)"$"#)]
async fn present(world: &mut QueryingWorld, text: String) {
    assert!(
        world.answer().text.contains(&text),
        "{}",
        world.answer().text
    );
}
