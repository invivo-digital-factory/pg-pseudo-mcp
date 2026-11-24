use cucumber::{then, when};
use scarif_database_mcp::composition::server_info::server_info;
use scarif_database_mcp::contexts::querying::domain::services::column_policy::field_hint;
use scarif_database_mcp::contexts::querying::domain::types::{
    ColumnSource, FieldSource, PolicyDecision, Reason,
};

use crate::querying_world::QueryingWorld;

fn field(collection: &str, path: &str) -> FieldSource {
    let (database, collection) = collection
        .split_once('.')
        .expect("database.collection in the scenario");
    FieldSource {
        database: database.to_string(),
        collection: collection.to_string(),
        path: path.to_string(),
    }
}

fn column(qualified: &str) -> ColumnSource {
    let parts: Vec<&str> = qualified.split('.').collect();
    ColumnSource {
        schema: parts[0].to_string(),
        table: parts[1].to_string(),
        column: parts[2].to_string(),
    }
}

fn reason(name: &str) -> Reason {
    match name {
        "dictionary" => Reason::Dictionary,
        "default" => Reason::Default,
        other => panic!("unknown reason {other}"),
    }
}

#[when(regex = r"^la politique de colonnes est chargée$")]
async fn policy_is_loaded(world: &mut QueryingWorld) {
    world.load_column_policy();
}

#[then(regex = r#"^le champ "([^"]+)" de "([^"]+)" est en clair$"#)]
async fn field_is_clear(world: &mut QueryingWorld, path: String, collection: String) {
    assert_eq!(
        world.policy.decide_field(&field(&collection, &path)),
        Some(&PolicyDecision::Clear)
    );
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

#[then(regex = r#"^la colonne "([^"]+)" est en clair$"#)]
async fn column_is_clear_by_policy(world: &mut QueryingWorld, qualified: String) {
    assert_eq!(
        world.policy.decide(&column(&qualified)),
        Some(&PolicyDecision::Clear)
    );
}

#[then(regex = r#"^la colonne "([^"]+)" n'est pas classée$"#)]
async fn column_is_unclassified(world: &mut QueryingWorld, qualified: String) {
    assert_eq!(world.policy.decide(&column(&qualified)), None);
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

#[then(regex = r#"^un avertissement "([^"]+)" nomme le fichier partagé$"#)]
async fn warning_names_shared_file(world: &mut QueryingWorld, code: String) {
    let info = serde_json::to_value(server_info(world.policy_warnings.clone(), Vec::new()))
        .expect("server info");
    let warnings = info["warnings"].as_array().expect("warnings array");
    assert_eq!(warnings.len(), 1, "{info}");
    assert_eq!(warnings[0]["code"], code.as_str(), "{info}");
    let message = warnings[0]["message"].as_str().expect("message");
    assert!(message.contains("team-columns.toml"), "{message}");
}

#[then(regex = r"^aucun avertissement de politique n'est signalé$")]
async fn no_policy_warning(world: &mut QueryingWorld) {
    assert!(
        world.policy_warnings.is_empty(),
        "{:?}",
        world.policy_warnings
    );
}
