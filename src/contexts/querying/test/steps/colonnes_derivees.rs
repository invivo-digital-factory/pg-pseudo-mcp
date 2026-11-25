use cucumber::{then, when};
use scarif_database_mcp::contexts::querying::domain::types::Lineage;
use serde_json::json;

use crate::querying_world::{CANARY_VALUES, QueryingWorld};

const REAL_FRAGMENTS: [&str; 4] = ["ylwenna", "kervadec", "canary-mail", "canary"];

#[then(regex = r"^la réponse ne contient aucune valeur canari$")]
async fn answer_has_no_canary(world: &mut QueryingWorld) {
    let text = world.answer().text.to_lowercase();
    for value in CANARY_VALUES {
        assert!(!text.contains(&value.to_lowercase()), "{value} in {text}");
    }
    assert!(!text.contains("canary-mail"), "{text}");
}

#[then(
    regex = r#"^les colonnes (\d+) et (\d+) sont pseudonymisées comme "([^"]+)" par le dictionnaire$"#
)]
async fn columns_flagged_by_dictionary(
    world: &mut QueryingWorld,
    first: usize,
    second: usize,
    kind: String,
) {
    let answer = world.answer();
    let body = answer.body();
    for position in [first, second] {
        let flag = &body["columns"][position - 1]["pseudonymized"];
        assert_eq!(flag["kind"], kind.as_str(), "{}", answer.text);
        assert_eq!(flag["why"], "dictionary", "{}", answer.text);
    }
}

#[then(regex = r"^la colonne (\d+) n'est pas pseudonymisée$")]
async fn column_not_flagged(world: &mut QueryingWorld, position: usize) {
    let answer = world.answer();
    let body = answer.body();
    let column = &body["columns"][position - 1];
    assert!(column.is_object(), "{}", answer.text);
    assert!(column.get("pseudonymized").is_none(), "{}", answer.text);
}

#[then(regex = r"^la sortie (\d+) est de lignée (personnelle|propre|inconnue)$")]
async fn output_lineage(world: &mut QueryingWorld, position: usize, expected: String) {
    let guarded = world
        .sql
        .as_ref()
        .expect("a statement was guarded")
        .as_ref()
        .unwrap_or_else(|refusal| panic!("refused: {refusal}"));
    let expected = match expected.as_str() {
        "personnelle" => Lineage::Personal,
        "propre" => Lineage::Clean,
        _ => Lineage::Unknown,
    };
    let lineage = guarded
        .derived_lineage
        .as_ref()
        .unwrap_or_else(|| panic!("no lineage for {}", guarded.sql));
    assert_eq!(
        lineage.get(position - 1),
        Some(&expected),
        "{}",
        guarded.sql
    );
}

#[then(regex = r#"^la colonne "([^"]+)" est signalée comme dérivée et pseudonymisée$"#)]
async fn column_is_flagged_derived(world: &mut QueryingWorld, column: String) {
    let answer = world.answer();
    assert_eq!(
        answer.body()["pseudonymized"][column.as_str()]["why"],
        "derived",
        "{}",
        answer.text
    );
}

#[then(regex = r#"^la colonne "([^"]+)" vaut "([^"]+)" en clair$"#)]
async fn column_is_clear(world: &mut QueryingWorld, column: String, expected: String) {
    let answer = world.answer();
    let body = answer.body();
    let index = body["columns"]
        .as_array()
        .expect("columns array")
        .iter()
        .position(|item| item["name"] == column.as_str())
        .unwrap_or_else(|| panic!("no column {column} in {}", answer.text));
    assert_eq!(
        answer.rows()[0][index],
        expected.as_str(),
        "{}",
        answer.text
    );
    assert!(
        body["pseudonymized"].get(column.as_str()).is_none(),
        "{}",
        answer.text
    );
}

#[then(regex = r"^la réponse ne contient aucun fragment de l'adresse réelle$")]
async fn answer_has_no_real_fragment(world: &mut QueryingWorld) {
    let text = world.answer().text.to_lowercase();
    for fragment in REAL_FRAGMENTS {
        assert!(!text.contains(fragment), "{fragment} leaked in {text}");
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
