use cucumber::{then, when};
use scarif_database_mcp::contexts::querying::domain::services::statement_guard::inspect;
use serde_json::{Value, json};

use crate::querying_world::QueryingWorld;

#[then(regex = r"^la première cellule de la réponse est un pseudonyme$")]
async fn first_cell_is_a_pseudonym(world: &mut QueryingWorld) {
    let answer = world.answer();
    assert!(!answer.refused, "{}", answer.text);
    let pseudonym = answer.rows()[0][0].as_str().expect("text cell").to_string();
    assert_eq!(answer.body()["pseudonymized"]["email"]["kind"], "email");
    assert!(!pseudonym.contains("canary-mail"), "{pseudonym}");
    world.pseudonym = Some(pseudonym);
}

#[when(regex = r"^l'agent cherche les commandes du client dont l'adresse est ce pseudonyme$")]
async fn agent_looks_up_orders(world: &mut QueryingWorld) {
    let pseudonym = world.pseudonym.clone().expect("a pseudonym was read");
    let sql = format!(
        "SELECT o.id FROM orders o JOIN customers c ON c.id = o.customer_id WHERE c.email = '{pseudonym}' ORDER BY o.id"
    );
    world.ask(&sql).await;
}

#[then(regex = r"^la réponse liste les commandes 10 et 11$")]
async fn answer_lists_orders(world: &mut QueryingWorld) {
    assert_eq!(world.answer().body()["rows"], json!([[10], [11]]));
}

#[then(regex = r"^le pseudonyme a été traduit en une valeur réelle côté serveur$")]
async fn pseudonym_was_translated(world: &mut QueryingWorld) {
    assert_eq!(world.answer().body()["translated_literals"], 1);
}

#[then(regex = r"^aucun pseudonyme n'a été traduit$")]
async fn no_pseudonym_translated(world: &mut QueryingWorld) {
    let answer = world.answer();
    assert!(!answer.refused, "{}", answer.text);
    assert_eq!(answer.body()["translated_literals"], 0, "{}", answer.text);
}

#[when(regex = r#"^l'agent lit la colonne "([a-z_]+)" du client 1$"#)]
async fn agent_reads_column(world: &mut QueryingWorld, column: String) {
    world
        .ask(&format!("SELECT {column} FROM customers WHERE id = 1"))
        .await;
}

#[then(regex = r#"^la colonne lue est pseudonymisée comme "([^"]+)"$"#)]
async fn read_column_is_pseudonymized(world: &mut QueryingWorld, kind: String) {
    let answer = world.answer();
    assert!(!answer.refused, "{}", answer.text);
    let body = answer.body();
    assert_eq!(
        body["columns"][0]["pseudonymized"]["kind"],
        kind.as_str(),
        "{}",
        answer.text
    );
    let cell = match &answer.rows()[0][0] {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    world.pseudonym = Some(cell);
}

#[when(regex = r#"^l'agent filtre les clients sur ce pseudonyme dans la colonne "([a-z_]+)"$"#)]
async fn agent_filters_on_pseudonym(world: &mut QueryingWorld, column: String) {
    let pseudonym = world.pseudonym.clone().expect("a pseudonym was read");
    world
        .ask(&format!(
            "SELECT id FROM customers WHERE {column} = '{pseudonym}' ORDER BY id"
        ))
        .await;
}

#[then(regex = r"^la réponse contient le client (\d+)$")]
async fn answer_contains_customer(world: &mut QueryingWorld, id: u64) {
    let answer = world.answer();
    assert!(!answer.refused, "{}", answer.text);
    assert!(answer.rows().contains(&vec![json!(id)]), "{}", answer.text);
}

#[then(regex = r#"^la réponse porte l'avertissement "([^"]+)"$"#)]
async fn answer_carries_warning(world: &mut QueryingWorld, code: String) {
    let answer = world.answer();
    let body = answer.body();
    let found = body["warnings"].as_array().is_some_and(|warnings| {
        warnings
            .iter()
            .any(|warning| warning["code"] == code.as_str())
    });
    assert!(found, "{}", answer.text);
}

#[then(regex = r"^la requête envoyée reste une seule lecture$")]
async fn sent_query_stays_one_read(world: &mut QueryingWorld) {
    let (_, _, sql) = world.accepted_translations();
    if let Err(refusal) = inspect(&sql) {
        panic!("{sql}: {} {}", refusal.code, refusal.message);
    }
}
