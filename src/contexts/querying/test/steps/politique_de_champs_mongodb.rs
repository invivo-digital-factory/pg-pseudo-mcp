use cucumber::then;
use scarif_database_mcp::contexts::querying::domain::services::column_policy::field_hint;
use scarif_database_mcp::contexts::querying::domain::types::{PolicyDecision, Reason};

use crate::querying_steps::support::field;
use crate::querying_world::QueryingWorld;

fn reason(name: &str) -> Reason {
    match name {
        "dictionary" => Reason::Dictionary,
        "default" => Reason::Default,
        other => panic!("unknown reason {other}"),
    }
}

#[then(regex = r#"^le champ "([^"]+)" de "([^"]+)" est personnel de type "([^"]+)"$"#)]
async fn field_is_personal(
    world: &mut QueryingWorld,
    path: String,
    collection: String,
    kind: String,
) {
    assert_eq!(
        world.policy.decide_field(&field(&collection, &path)),
        Some(&PolicyDecision::Personal(kind))
    );
}

#[then(regex = r#"^le champ "([^"]+)" de "([^"]+)" n'est pas classé$"#)]
async fn field_is_unclassified(world: &mut QueryingWorld, path: String, collection: String) {
    assert_eq!(world.policy.decide_field(&field(&collection, &path)), None);
}

#[then(
    regex = r#"^le champ "([^"]+)" de "([^"]+)" pseudonymisé comme "([^"]+)" par le dictionnaire ne reçoit aucun indice$"#
)]
async fn dictionary_field_gets_no_hint(
    world: &mut QueryingWorld,
    path: String,
    collection: String,
    kind: String,
) {
    world.hint = field_hint(
        &world.policy,
        &field(&collection, &path),
        Reason::Dictionary,
        &kind,
    );
    assert_eq!(world.hint, None);
}

#[then(
    regex = r#"^le champ "([^"]+)" de "([^"]+)" pseudonymisé comme "([^"]+)" par le dictionnaire reçoit l'indice de collection "([^"]+)", de champ "([^"]+)" et de raison "([^"]+)"$"#
)]
async fn dictionary_field_gets_hint(
    world: &mut QueryingWorld,
    path: String,
    collection: String,
    kind: String,
    hint_collection: String,
    hint_field: String,
    hint_reason: String,
) {
    let hint = field_hint(
        &world.policy,
        &field(&collection, &path),
        Reason::Dictionary,
        &kind,
    )
    .expect("a hint");
    assert_eq!(hint.collection, hint_collection);
    assert_eq!(hint.field, hint_field);
    assert_eq!(hint.reason, reason(&hint_reason));
}

#[then(regex = r"^aucun avertissement de politique n'est signalé$")]
async fn no_policy_warning(world: &mut QueryingWorld) {
    assert!(
        world.policy_warnings.is_empty(),
        "{:?}",
        world.policy_warnings
    );
}
