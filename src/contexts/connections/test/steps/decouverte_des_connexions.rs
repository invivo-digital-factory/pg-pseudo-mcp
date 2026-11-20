use cucumber::{given, then, when};
use pg_pseudo_mcp::contexts::connections::presentation;
use serde_json::Value;

use crate::bdd_support::{offline_target, text_of, write_workspace};
use crate::connections_world::ConnectionsWorld;

#[given(
    regex = r#"^un espace de travail DBeaver où la connexion "([^"]+)" a son mot de passe enregistré$"#
)]
async fn workspace_with_saved_password(world: &mut ConnectionsWorld, _name: String) {
    write_workspace(world.workspace.path(), &offline_target(), true, None);
}

#[given(
    regex = r#"^un espace de travail DBeaver où la connexion "([^"]+)" n'a pas de mot de passe enregistré$"#
)]
async fn workspace_without_saved_password(world: &mut ConnectionsWorld, _name: String) {
    write_workspace(world.workspace.path(), &offline_target(), false, None);
}

#[when(regex = r"^l'agent liste les connexions$")]
async fn agent_lists_connections(world: &mut ConnectionsWorld) {
    let context = world.context();
    let result = presentation::list_connections(&context.list_connections).await;
    world.listing = Some(serde_json::from_str(&text_of(&result)).expect("json listing"));
}

fn listed<'a>(world: &'a ConnectionsWorld, name: &str) -> &'a Value {
    world.listing.as_ref().expect("connections were listed")["connections"]
        .as_array()
        .expect("connections array")
        .iter()
        .find(|connection| connection["name"] == name)
        .unwrap_or_else(|| panic!("connection {name} is not listed"))
}

#[then(regex = r#"^la connexion "([^"]+)" est listée comme disponible$"#)]
async fn connection_is_available(world: &mut ConnectionsWorld, name: String) {
    assert_eq!(listed(world, &name)["status"], "available");
}

#[then(regex = r#"^la connexion "([^"]+)" est listée comme indisponible$"#)]
async fn connection_is_unavailable(world: &mut ConnectionsWorld, name: String) {
    assert_eq!(listed(world, &name)["status"], "unavailable");
}

#[then(regex = r"^la liste ne montre ni hôte, ni utilisateur, ni mot de passe$")]
async fn listing_hides_credentials(world: &mut ConnectionsWorld) {
    let raw = world.listing.as_ref().expect("listed").to_string();
    for secret in ["db.internal.example", "reader", "reader-secret"] {
        assert!(!raw.contains(secret), "{secret} leaked in {raw}");
    }
}

#[then(regex = r"^la raison indique qu'aucun mot de passe n'est enregistré dans DBeaver$")]
async fn reason_names_missing_password(world: &mut ConnectionsWorld) {
    let reason = listed(world, "canary-local")["reason"]
        .as_str()
        .expect("reason")
        .to_string();
    assert!(reason.contains("no saved password"), "{reason}");
}
