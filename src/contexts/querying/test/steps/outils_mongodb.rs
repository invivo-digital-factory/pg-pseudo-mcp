use cucumber::{given, then, when};
use scarif_database_mcp::composition::tools;
use scarif_database_mcp::contexts::querying::presentation::{AggregateArgs, FindArgs, QueryArgs};
use serde_json::{Value, json};

use crate::querying_world::{MONGO_CONNECTION, QueryingWorld};

#[given(regex = r"^la connexion MongoDB de test à travers le bastion$")]
async fn mongo_connection(world: &mut QueryingWorld) {
    world.connect_to_test_mongodb();
}

#[when(regex = r#"^l'agent exécute la requête SQL "(.+)" sur la connexion MongoDB$"#)]
async fn sql_on_mongo(world: &mut QueryingWorld, sql: String) {
    let result = tools::query(
        world.contexts(),
        QueryArgs {
            connection: MONGO_CONNECTION.into(),
            sql,
            ..QueryArgs::default()
        },
    )
    .await;
    world.record(&result);
}

async fn find(world: &mut QueryingWorld, collection: &str, filter: Value) {
    let result = tools::find(
        world.contexts(),
        FindArgs {
            connection: MONGO_CONNECTION.into(),
            collection: collection.into(),
            filter: Some(filter),
            ..FindArgs::default()
        },
    )
    .await;
    world.record(&result);
}

#[when(regex = r#"^l'agent cherche dans "([^"]+)" le document d'identifiant "([0-9a-f]{24})"$"#)]
async fn find_by_id(world: &mut QueryingWorld, collection: String, id: String) {
    find(world, &collection, json!({"_id": {"$oid": id}})).await;
}

#[when(
    regex = r#"^l'agent cherche dans "([^"]+)" les documents dont "([^"]+)" vaut ce pseudonyme$"#
)]
async fn find_by_pseudonym(world: &mut QueryingWorld, collection: String, path: String) {
    let pseudonym = world.pseudonym.clone().expect("a pseudonym was seen");
    find(world, &collection, json!({ path: pseudonym })).await;
}

#[when(regex = r"^l'agent agrège (.+) avec (\[.*\])$")]
async fn aggregate(world: &mut QueryingWorld, collection: String, pipeline: String) {
    let pipeline: Value = serde_json::from_str(&pipeline).expect("a JSON pipeline");
    let result = tools::aggregate(
        world.contexts(),
        AggregateArgs {
            connection: MONGO_CONNECTION.into(),
            collection: collection.trim_matches('"').into(),
            pipeline: pipeline.as_array().expect("a pipeline").clone(),
            ..AggregateArgs::default()
        },
    )
    .await;
    world.record(&result);
}

#[then(regex = r#"^le champ "([^"]+)" est pseudonymisé comme "([^"]+)"$"#)]
async fn flagged(world: &mut QueryingWorld, path: String, kind: String) {
    let body = world.answer().body();
    assert_eq!(body["pseudonymized"][&path]["kind"], kind, "{body}");
    let value = path
        .split('.')
        .fold(&body["documents"][0], |node, key| &node[key])
        .as_str()
        .expect("a text value")
        .to_string();
    world.pseudonym = Some(value);
}

#[then(regex = r"^la réponse compte (\d+) documents$")]
async fn document_count(world: &mut QueryingWorld, count: usize) {
    let body = world.answer().body();
    assert_eq!(body["document_count"], count, "{body}");
}

#[then(regex = r"^un pseudonyme a été retraduit$")]
async fn translated(world: &mut QueryingWorld) {
    let body = world.answer().body();
    assert_eq!(body["translated_literals"], 1, "{body}");
}

#[then(regex = r#"^l'outil est refusé et renvoie vers "([^"]+)"$"#)]
async fn redirected(world: &mut QueryingWorld, tool: String) {
    let answer = world.answer();
    assert!(answer.refused, "{}", answer.text);
    let body = answer.body();
    assert_eq!(body["error"]["code"], "wrong_engine");
    assert!(
        body["error"]["message"].as_str().unwrap().contains(&tool),
        "{body}"
    );
}

#[then(regex = r#"^l'outil est refusé avec le code "([^"]+)"$"#)]
async fn refused_with(world: &mut QueryingWorld, code: String) {
    let answer = world.answer();
    assert!(answer.refused, "{}", answer.text);
    assert_eq!(answer.body()["error"]["code"], code);
}

#[then(regex = r#"^la réponse ne contient pas "([^"]+)"$"#)]
async fn absent(world: &mut QueryingWorld, text: String) {
    assert!(
        !world.answer().text.contains(&text),
        "{}",
        world.answer().text
    );
}
