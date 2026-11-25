use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use scarif_database_mcp::contexts::querying::domain::services::statement_guard::inspect;
use scarif_database_mcp::contexts::querying::domain::types::{GuardedMongo, MongoCommand};
use scarif_database_mcp::contexts::querying::presentation::FindArgs;
use serde_json::{Value, json};

use crate::querying_steps::support::{checked, find};
use crate::querying_world::QueryingWorld;

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

#[then(regex = r"^la requête envoyée reste une seule lecture$")]
async fn sent_query_stays_one_read(world: &mut QueryingWorld) {
    let (_, _, sql) = world.accepted_translations();
    if let Err(refusal) = inspect(&sql) {
        panic!("{sql}: {} {}", refusal.code, refusal.message);
    }
}

#[when(
    regex = r"^l'agent affiche ce pseudonyme à côté du statut des commandes filtrées sur ce pseudonyme$"
)]
async fn agent_projects_pseudonym(world: &mut QueryingWorld) {
    let pseudonym = world.pseudonym.clone().expect("a pseudonym was read");
    let sql = format!(
        "SELECT concat(o.status, ' ', '{pseudonym}') AS etiquette FROM orders o JOIN customers c ON c.id = o.customer_id WHERE c.email = '{pseudonym}' ORDER BY o.id"
    );
    world.ask(&sql).await;
}

#[then(regex = r"^chaque ligne montre le statut suivi du pseudonyme, en clair$")]
async fn rows_show_status_and_pseudonym(world: &mut QueryingWorld) {
    let pseudonym = world.pseudonym.clone().expect("a pseudonym was read");
    let answer = world.answer();
    assert_eq!(
        answer.body()["rows"],
        json!([
            [format!("paid {pseudonym}")],
            [format!("shipped {pseudonym}")]
        ]),
        "{}",
        answer.text
    );
    assert!(
        answer.body().get("pseudonymized").is_none(),
        "{}",
        answer.text
    );
}

#[when(
    regex = r#"^l'agent cherche dans "([^"]+)" les documents dont "([^"]+)" vaut ce pseudonyme$"#
)]
async fn find_by_pseudonym(world: &mut QueryingWorld, collection: String, path: String) {
    let pseudonym = world.pseudonym.clone().expect("a pseudonym was seen");
    find(
        world,
        FindArgs {
            collection,
            filter: Some(json!({ path: pseudonym })),
            ..FindArgs::default()
        },
    )
    .await;
}

#[then(regex = r#"^le champ "([^"]+)" est pseudonymisé comme "([^"]+)"$"#)]
async fn flagged(world: &mut QueryingWorld, path: String, kind: String) {
    let body = checked(world);
    assert_eq!(body["pseudonymized"][&path]["kind"], kind, "{body}");
    let value = path
        .split('.')
        .fold(&body["documents"][0], |node, key| &node[key])
        .as_str()
        .expect("a text value")
        .to_string();
    world.pseudonym = Some(value);
}

#[then(regex = r"^un pseudonyme a été retraduit$")]
async fn translated(world: &mut QueryingWorld) {
    let body = checked(world);
    assert_eq!(body["translated_literals"], 1, "{body}");
}

fn accepted(world: &QueryingWorld) -> GuardedMongo {
    world
        .mongo_answer()
        .outcome
        .clone()
        .unwrap_or_else(|refusal| panic!("refused: {refusal}"))
}

fn sent_filter(world: &QueryingWorld) -> Value {
    match accepted(world).command {
        MongoCommand::Find(command) => command.filter,
        MongoCommand::Aggregate(command) => Value::Array(command.pipeline),
    }
}

#[given(regex = r#"^le pseudonyme "([^"]+)" désigne la valeur réelle "([^"]+)"$"#)]
async fn pseudonym_maps_to(world: &mut QueryingWorld, pseudonym: String, real: String) {
    world.pseudonyms.push((pseudonym, real));
}

#[then(regex = r"^(?:le filtre|la requête) envoyée? ne contient plus le pseudonyme$")]
async fn filter_has_no_pseudonym(world: &mut QueryingWorld) {
    let (_, _, text) = world.accepted_translations();
    for (pseudonym, real) in &world.pseudonyms {
        assert!(!text.contains(pseudonym.as_str()), "{text}");
        assert!(text.contains(real.as_str()), "{text}");
    }
}

#[then(regex = r#"^(?:le filtre|la requête) envoyée? contient "(.+)"$"#)]
async fn sent_text_contains(world: &mut QueryingWorld, expected: String) {
    let (_, _, text) = world.accepted_translations();
    assert!(text.contains(&expected), "{text}");
}

#[then(regex = r#"^(?:le filtre|la requête) envoyée? ne contient pas "(.+)"$"#)]
async fn sent_text_omits(world: &mut QueryingWorld, unexpected: String) {
    let (_, _, text) = world.accepted_translations();
    assert!(!text.contains(&unexpected), "{text}");
}

#[then(regex = r#"^la demande porte l'avertissement "([^"]+)"$"#)]
async fn request_carries_warning(world: &mut QueryingWorld, code: String) {
    let (_, warnings, _) = world.accepted_translations();
    assert!(
        warnings.iter().any(|warning| warning.code == code),
        "{warnings:?}"
    );
}

#[then(regex = r"^la demande ne porte aucun avertissement$")]
async fn request_has_no_warning(world: &mut QueryingWorld) {
    let (_, warnings, _) = world.accepted_translations();
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[then(regex = r"^le filtre envoyé vaut :$")]
async fn filter_equals(world: &mut QueryingWorld, step: &Step) {
    let expected: Value =
        serde_json::from_str(step.docstring.as_deref().expect("a docstring")).expect("JSON");
    assert_eq!(sent_filter(world), expected);
}
