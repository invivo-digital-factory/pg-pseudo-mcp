use cucumber::then;

use crate::querying_world::QueryingWorld;

const REAL_FRAGMENTS: [&str; 4] = ["ylwenna", "kervadec", "canary-mail", "canary"];

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
