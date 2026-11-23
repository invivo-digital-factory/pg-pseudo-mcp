use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use serde_json::Value;

use crate::querying_world::QueryingWorld;

fn docstring(step: &Step) -> String {
    step.docstring
        .as_deref()
        .expect("a docstring")
        .trim_start_matches('\n')
        .to_string()
}

#[given(regex = r"^la politique de colonnes partagée suivante :$")]
async fn shared_policy(world: &mut QueryingWorld, step: &Step) {
    world.shared_policy = Some(docstring(step));
}

#[given(regex = r"^la politique de colonnes locale suivante :$")]
async fn local_policy(world: &mut QueryingWorld, step: &Step) {
    world.local_policy = Some(docstring(step));
}

#[when(regex = r#"^l'agent décrit la table "([^"]+)"$"#)]
async fn agent_describes(world: &mut QueryingWorld, table: String) {
    world.describe(&table).await;
}

#[when(regex = r"^l'agent liste les connexions$")]
async fn agent_lists_connections(world: &mut QueryingWorld) {
    world.list_connections().await;
}

#[then(regex = r#"^la colonne "([^"]+)" est pseudonymisée comme "([^"]+)" par la politique$"#)]
async fn column_pseudonymized_by_policy(world: &mut QueryingWorld, column: String, kind: String) {
    let answer = world.answer();
    let flag = &answer.body()["pseudonymized"][column.as_str()];
    assert_eq!(flag["kind"], kind.as_str(), "{}", answer.text);
    assert_eq!(flag["why"], "policy", "{}", answer.text);
}

#[then(regex = r#"^la colonne "([^"]+)" ne laisse voir aucune adresse e-mail réelle$"#)]
async fn column_hides_email(world: &mut QueryingWorld, column: String) {
    let answer = world.answer();
    let body = answer.body();
    assert_eq!(
        body["pseudonymized"][column.as_str()]["why"],
        "detected",
        "{}",
        answer.text
    );
    assert!(!answer.text.contains("canary-mail"), "{}", answer.text);
}

fn hinted(body: &Value) -> Vec<String> {
    body.get("column_policy_hints")
        .and_then(Value::as_array)
        .map(|hints| {
            hints
                .iter()
                .map(|hint| hint["column"].as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[then(regex = r#"^la réponse propose de classer la colonne "([^"]+)"$"#)]
async fn answer_hints_column(world: &mut QueryingWorld, column: String) {
    let answer = world.answer();
    assert!(hinted(&answer.body()).contains(&column), "{}", answer.text);
}

#[then(regex = r"^la réponse ne propose de classer aucune colonne$")]
async fn answer_has_no_hint(world: &mut QueryingWorld) {
    let answer = world.answer();
    assert!(hinted(&answer.body()).is_empty(), "{}", answer.text);
}

#[then(regex = r"^la liste signale que la politique partagée est ignorée$")]
async fn listing_warns_policy(world: &mut QueryingWorld) {
    let answer = world.answer();
    let body = answer.body();
    assert_eq!(
        body["warnings"][0]["code"], "column_policy_ignored",
        "{}",
        answer.text
    );
    assert!(
        body["warnings"][0]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("team-columns.toml"),
        "{}",
        answer.text
    );
}
