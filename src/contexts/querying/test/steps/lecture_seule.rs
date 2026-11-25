use cucumber::{given, then, when};
use scarif_database_mcp::contexts::anonymization::use_cases::dictionary_kind::DictionaryKind;
use scarif_database_mcp::contexts::querying::domain::repositories::PseudonymTranslator;
use scarif_database_mcp::contexts::querying::domain::services::statement_guard::inspect;
use scarif_database_mcp::contexts::querying::domain::types::{
    Guarded, PersonalNames, Translations,
};
use serde_json::json;

use crate::querying_world::{CANARY_VALUES, PseudonymBook, QueryingWorld};

fn refusal(code: String, message: String, hint: Option<String>) -> String {
    format!("{code} {message} {}", hint.unwrap_or_default())
}

fn guard_sql(sql: &str, store: &PseudonymBook) -> Result<Guarded, String> {
    let inspected = inspect(sql).map_err(|error| refusal(error.code, error.message, error.hint))?;
    let mut translations = Translations::default();
    for literal in inspected.literals() {
        translations.record(literal, store.lookup(literal));
    }
    let rewritten = inspected
        .rewrite(&translations)
        .map_err(|error| refusal(error.code, error.message, error.hint))?;
    let mut personal = PersonalNames::default();
    for name in rewritten.lineage_names() {
        if DictionaryKind.execute(&name).is_some() {
            personal.insert(&name);
        }
    }
    Ok(rewritten.finish(&personal))
}

#[given(regex = r"^l'agent connecté à la base de test$")]
async fn agent_connected(world: &mut QueryingWorld) {
    world.connect_to_test_database();
}

#[given(regex = r"^une connexion PostgreSQL vers une base injoignable$")]
async fn unreachable_database(world: &mut QueryingWorld) {
    world.connect_to_unreachable_database();
}

#[when(regex = r#"^l'agent exécute la requête "(.+)"$"#)]
async fn agent_runs_query(world: &mut QueryingWorld, sql: String) {
    world.ask(&sql).await;
}

#[when(regex = r#"^la garde SQL examine la requête "(.+)"$"#)]
async fn sql_guard_examines(world: &mut QueryingWorld, sql: String) {
    let store = PseudonymBook::new(&world.pseudonyms);
    world.sql = Some(guard_sql(&sql, &store));
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

#[then(regex = r#"^la requête est refusée par la règle "([^"]+)"$"#)]
async fn query_is_refused_by_rule(world: &mut QueryingWorld, rule: String) {
    let answer = world.answer();
    assert!(answer.refused, "{}", answer.text);
    let error = &answer.body()["error"];
    assert_eq!(error["category"], "refused_read_only", "{}", answer.text);
    assert_eq!(error["code"], rule.as_str(), "{}", answer.text);
}

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

#[then(regex = r"^la première cellule de la réponse vaut (\d+)$")]
async fn first_cell_equals(world: &mut QueryingWorld, expected: u64) {
    assert_eq!(world.answer().rows()[0][0], json!(expected));
}
