use cucumber::then;
use scarif_database_mcp::contexts::connections::domain::errors::ConnectionError;

use crate::connections_steps::support::{connections, listed};
use crate::connections_world::ConnectionsWorld;

#[then(regex = r#"^la connexion "([^"]+)" est listée sans raison$"#)]
async fn connection_has_no_reason(world: &mut ConnectionsWorld, name: String) {
    let entry = listed(world, &name);
    assert!(entry.get("reason").is_none(), "{entry}");
}

#[then(regex = r#"^la connexion "([^"]+)" est listée exactement avec les champs "([^"]+)"$"#)]
async fn connection_has_exactly(world: &mut ConnectionsWorld, name: String, fields: String) {
    let entry = listed(world, &name);
    let mut actual: Vec<&str> = entry
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    let mut expected: Vec<&str> = fields.split(", ").collect();
    actual.sort_unstable();
    expected.sort_unstable();
    assert_eq!(actual, expected, "{entry}");
}

#[then(regex = r"^la liste ne montre pas le chemin du dossier personnel$")]
async fn listing_hides_home(world: &mut ConnectionsWorld) {
    let home = std::env::var("HOME").expect("HOME");
    let raw = world.listing.as_ref().expect("listed").to_string();
    assert!(!raw.contains(&home), "{home} leaked in {raw}");
}

#[then(regex = r#"^les connexions sont listées dans l'ordre "([^"]+)"$"#)]
async fn connections_in_order(world: &mut ConnectionsWorld, order: String) {
    let names: Vec<&str> = connections(world)
        .iter()
        .map(|connection| connection["name"].as_str().expect("name"))
        .collect();
    assert_eq!(names, order.split(", ").collect::<Vec<_>>());
}

#[then(regex = r#"^l'erreur propose les connexions disponibles "([^"]+)"$"#)]
async fn error_lists_available(world: &mut ConnectionsWorld, expected: String) {
    let expected: Vec<String> = expected.split(", ").map(str::to_string).collect();
    match &world.failure {
        Some(ConnectionError::Unknown { available, .. }) => assert_eq!(available, &expected),
        other => panic!("expected an unknown connection error, got {other:?}"),
    }
}
