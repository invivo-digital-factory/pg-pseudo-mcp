use cucumber::gherkin::Step;
use cucumber::{given, then};
use serde_json::{Value, json};

use crate::bdd_support::{CONNECTION, offline_target, write_workspace};
use crate::connections_steps::support::{
    connections, dbeaver_dir, list, listed, params, postgres_connection, reason, saved,
    tunnel_connection, write_credentials, write_dbeaver,
};
use crate::connections_world::ConnectionsWorld;

#[given(
    regex = r#"^un espace de travail DBeaver où la connexion "([^"]+)" n'a pas de mot de passe enregistré$"#
)]
async fn workspace_without_saved_password(world: &mut ConnectionsWorld, name: String) {
    assert_eq!(
        name, CONNECTION,
        "the shared workspace names its connection {CONNECTION}"
    );
    write_workspace(world.workspace.path(), &offline_target(), false, None);
}

#[given(regex = r"^un espace de travail DBeaver dont le fichier data-sources.json est :$")]
async fn workspace_with_sources(world: &mut ConnectionsWorld, step: &Step) {
    let sources: Value =
        serde_json::from_str(step.docstring.as_deref().expect("a docstring")).expect("valid JSON");
    let connections: Vec<(String, Value, Value)> = sources["connections"]
        .as_object()
        .expect("connections object")
        .iter()
        .map(|(id, connection)| (id.clone(), connection.clone(), Value::Null))
        .collect();
    write_dbeaver(world, "General", "", &connections);
}

#[given(regex = r"^dont les identifiants enregistrés sont :$")]
async fn workspace_credentials(world: &mut ConnectionsWorld, step: &Step) {
    let credentials: Value =
        serde_json::from_str(step.docstring.as_deref().expect("a docstring")).expect("valid JSON");
    write_credentials(world, "General", "", &credentials);
}

#[given(regex = r#"^un second fichier data-sources-2.json qui déclare la connexion "([^"]+)"$"#)]
async fn secondary_sources(world: &mut ConnectionsWorld, name: String) {
    let connection = postgres_connection(
        &name,
        json!({ "host": "db.example.com", "database": "shop", "type": "dev" }),
    );
    write_dbeaver(
        world,
        "General",
        "-2",
        &[("pg-2".to_string(), connection, saved("alice", "s3cret"))],
    );
}

#[given(
    regex = r#"^un projet DBeaver "([^"]+)" dont le fichier data-sources.json n'est pas du JSON$"#
)]
async fn unreadable_project(world: &mut ConnectionsWorld, project: String) {
    let dir = dbeaver_dir(world, &project);
    std::fs::create_dir_all(&dir).expect("workspace dir");
    std::fs::write(dir.join("data-sources.json"), [0x8a, 0x01, 0xff, 0x00])
        .expect("encrypted data sources");
}

#[given(
    regex = r#"^un espace de travail DBeaver dont la connexion "([^"]+)" a le gestionnaire réseau suivant :$"#
)]
async fn workspace_with_handler(world: &mut ConnectionsWorld, name: String, step: &Step) {
    let handlers: Value =
        serde_json::from_str(step.docstring.as_deref().expect("a docstring")).expect("valid JSON");
    tunnel_connection(world, &name, handlers, json!({ "user": "jump" }));
}

#[given(
    regex = r#"^un espace de travail DBeaver dont la connexion "([^"]+)" passe par un tunnel SSH qui contourne la vérification de clé d'hôte : (oui|non)$"#
)]
async fn workspace_with_bypass(world: &mut ConnectionsWorld, name: String, bypass: String) {
    let properties = json!({ "authType": "PASSWORD", "host": "bastion.example.com", "port": 22,
                             "bypassHostVerification": bypass == "oui" });
    tunnel_connection(
        world,
        &name,
        json!({ "ssh_tunnel": { "type": "TUNNEL", "enabled": true, "properties": properties } }),
        json!({ "user": "jump", "password": "s3cret" }),
    );
}

#[then(regex = r#"^la connexion "([^"]+)" est listée avec le statut "([^"]+)"$"#)]
async fn connection_has_status(world: &mut ConnectionsWorld, name: String, status: String) {
    let entry = listed(world, &name);
    assert_eq!(entry["status"], status, "{entry}");
}

#[then(regex = r"^la liste ne contient aucune connexion$")]
async fn no_connection_listed(world: &mut ConnectionsWorld) {
    assert!(connections(world).is_empty(), "{:?}", world.listing);
}

#[then(regex = r"^la raison indique qu'aucun mot de passe n'est enregistré dans DBeaver$")]
async fn reason_names_missing_password(world: &mut ConnectionsWorld) {
    let reason = reason(world, CONNECTION);
    assert!(reason.contains("no saved password"), "{reason}");
}

#[then(
    regex = r#"^deux connexions sont listées sous le nom "([^"]+)" suivi d'un suffixe distinct$"#
)]
async fn suffixed_duplicates(world: &mut ConnectionsWorld, name: String) {
    let mut names: Vec<String> = connections(world)
        .iter()
        .filter_map(|connection| connection["name"].as_str())
        .filter(|listed| listed.starts_with(&format!("{name} (")) && listed.ends_with(')'))
        .map(str::to_string)
        .collect();
    assert_eq!(names.len(), 2, "{:?}", world.listing);
    names.dedup();
    assert_eq!(names.len(), 2, "the suffixes differ");
    world.probe = Some(json!(names));
}

#[then(regex = r"^une nouvelle liste donne les mêmes noms suffixés$")]
async fn suffixes_are_stable(world: &mut ConnectionsWorld) {
    let before = world.probe.clone().expect("suffixed names");
    list(world).await;
    let after: Vec<String> = connections(world)
        .iter()
        .filter_map(|connection| connection["name"].as_str())
        .map(str::to_string)
        .collect();
    assert_eq!(json!(after), before);
}

#[then(regex = r#"^la connexion "([^"]+)" vise l'hôte "([^"]+)"$"#)]
async fn targets_host(world: &mut ConnectionsWorld, name: String, host: String) {
    assert_eq!(params(world, &name).host, host);
}

#[then(
    regex = r#"^la connexion "([^"]+)" se connecte en tant que "([^"]+)" avec le mot de passe "([^"]+)"$"#
)]
async fn connects_as(world: &mut ConnectionsWorld, name: String, user: String, password: String) {
    let params = params(world, &name);
    assert_eq!(params.user, user);
    assert_eq!(params.password.expose(), password);
}

#[then(regex = r#"^la connexion "([^"]+)" n'a pas de tunnel SSH$"#)]
async fn has_no_tunnel(world: &mut ConnectionsWorld, name: String) {
    assert!(params(world, &name).tunnel.is_none());
}
