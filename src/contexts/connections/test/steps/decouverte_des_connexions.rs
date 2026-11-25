use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use scarif_database_mcp::contexts::connections::domain::repositories::ConnectionCatalog;
use scarif_database_mcp::contexts::connections::infrastructure::data_sources::dbeaver::credentials::encrypt_for_tests;
use scarif_database_mcp::contexts::connections::presentation;
use serde_json::{Map, Value, json};

use crate::bdd_support::{CONNECTION, offline_target, ssh_asset, text_of, write_workspace};
use crate::connections_world::ConnectionsWorld;

pub fn write_dbeaver(
    world: &ConnectionsWorld,
    project: &str,
    suffix: &str,
    connections: &[(String, Value, Value)],
) {
    let dir = world
        .workspace
        .path()
        .join("workspace6")
        .join(project)
        .join(".dbeaver");
    std::fs::create_dir_all(&dir).expect("workspace dir");
    let mut sources = Map::new();
    let mut credentials = Map::new();
    for (id, connection, saved) in connections {
        sources.insert(id.clone(), connection.clone());
        if !saved.is_null() {
            credentials.insert(id.clone(), saved.clone());
        }
    }
    std::fs::write(
        dir.join(format!("data-sources{suffix}.json")),
        json!({ "folders": {}, "connections": sources }).to_string(),
    )
    .expect("data sources");
    write_credentials(world, project, suffix, &Value::Object(credentials));
}

fn write_credentials(world: &ConnectionsWorld, project: &str, suffix: &str, credentials: &Value) {
    let dir = world
        .workspace
        .path()
        .join("workspace6")
        .join(project)
        .join(".dbeaver");
    std::fs::write(
        dir.join(format!("credentials-config{suffix}.json")),
        encrypt_for_tests(&credentials.to_string(), [7u8; 16]),
    )
    .expect("credentials");
}

pub fn postgres_connection(name: &str, configuration: Value) -> Value {
    json!({
        "provider": "postgresql",
        "driver": "postgres-jdbc",
        "name": name,
        "save-password": true,
        "configuration": configuration
    })
}

fn saved(user: &str, password: &str) -> Value {
    json!({ "#connection": { "user": user, "password": password } })
}

#[given(
    regex = r#"^un espace de travail DBeaver où la connexion "([^"]+)" a son mot de passe enregistré$"#
)]
async fn workspace_with_saved_password(world: &mut ConnectionsWorld, name: String) {
    assert_eq!(
        name, CONNECTION,
        "the shared workspace names its connection {CONNECTION}"
    );
    write_workspace(world.workspace.path(), &offline_target(), true, None);
}

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
    let dir = world
        .workspace
        .path()
        .join("workspace6")
        .join(project)
        .join(".dbeaver");
    std::fs::create_dir_all(&dir).expect("workspace dir");
    std::fs::write(dir.join("data-sources.json"), [0x8a, 0x01, 0xff, 0x00])
        .expect("encrypted data sources");
}

#[given(
    regex = r#"^un espace de travail DBeaver avec une connexion "([^"]+)" du fournisseur "([^"]+)" au modèle d'authentification "([^"]+)"$"#
)]
async fn workspace_with_provider(
    world: &mut ConnectionsWorld,
    name: String,
    provider: String,
    model: String,
) {
    let mut connection = postgres_connection(
        &name,
        json!({ "host": "db.example.com", "database": "shop", "type": "dev", "auth-model": model }),
    );
    connection["provider"] = json!(provider);
    write_dbeaver(
        world,
        "General",
        "",
        &[("conn-1".to_string(), connection, saved("alice", "s3cret"))],
    );
}

#[given(
    regex = r#"^un espace de travail DBeaver avec une connexion "([^"]+)" au modèle d'authentification "([^"]+)" vers l'hôte "([^"]+)"$"#
)]
async fn workspace_with_auth_model(
    world: &mut ConnectionsWorld,
    name: String,
    model: String,
    host: String,
) {
    let connection = postgres_connection(
        &name,
        json!({ "host": host, "database": "shop", "type": "dev", "auth-model": model }),
    );
    write_dbeaver(
        world,
        "General",
        "",
        &[(
            "pg-1".to_string(),
            connection,
            saved("alice@example.com", "mot-de-passe-dbeaver"),
        )],
    );
}

#[given(
    regex = r#"^un espace de travail DBeaver qui déclare la connexion "([^"]+)" vers la base "([^"]+)" en mode pgpass$"#
)]
async fn workspace_with_pgpass_connection(
    world: &mut ConnectionsWorld,
    name: String,
    database: String,
) {
    let connection = postgres_connection(
        &name,
        json!({ "host": "db.example.com", "port": "5432", "database": database, "type": "dev",
                "auth-model": "postgres_pgpass", "user": "alice" }),
    );
    write_dbeaver(
        world,
        "General",
        "",
        &[("pg-1".to_string(), connection, Value::Null)],
    );
}

pub fn tunnel_connection(world: &ConnectionsWorld, name: &str, handlers: Value, ssh: Value) {
    let connection = postgres_connection(
        name,
        json!({ "host": "db.example.com", "database": "shop", "type": "dev",
                "user": "alice", "password": "s3cret", "handlers": handlers }),
    );
    let mut credentials = json!({});
    if !ssh.is_null() {
        credentials["network/ssh_tunnel"] = ssh;
    }
    write_dbeaver(
        world,
        "General",
        "",
        &[("pg-tunnel".to_string(), connection, credentials)],
    );
}

#[given(
    regex = r#"^un espace de travail DBeaver dont la connexion "([^"]+)" passe par un tunnel SSH en "([^"]+)" avec la clé "([^"]*)", l'utilisateur SSH "([^"]*)" et le mot de passe SSH "([^"]*)"$"#
)]
async fn workspace_with_tunnel(
    world: &mut ConnectionsWorld,
    name: String,
    auth: String,
    key: String,
    user: String,
    password: String,
) {
    let home = std::env::var("HOME").unwrap_or_default();
    let key = key
        .replace(
            "{clé de test}",
            &ssh_asset("tunnel_test_ed25519").to_string_lossy(),
        )
        .replace("{dossier personnel}", &home);
    let mut properties = json!({ "authType": auth, "host": "bastion.example.com", "port": 22 });
    if !key.is_empty() {
        properties["keyPath"] = json!(key);
    }
    let mut ssh = Map::new();
    if !user.is_empty() {
        ssh.insert("user".to_string(), json!(user));
    }
    if !password.is_empty() {
        ssh.insert("password".to_string(), json!(password));
    }
    tunnel_connection(
        world,
        &name,
        json!({ "ssh_tunnel": { "type": "TUNNEL", "enabled": true, "properties": properties } }),
        Value::Object(ssh),
    );
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

#[given(regex = r#"^un espace de travail DBeaver avec deux connexions nommées "([^"]+)"$"#)]
async fn workspace_with_duplicates(world: &mut ConnectionsWorld, name: String) {
    let connections: Vec<(String, Value, Value)> = ["pg-1", "pg-2"]
        .into_iter()
        .map(|id| {
            (
                id.to_string(),
                postgres_connection(
                    &name,
                    json!({ "host": "db.example.com", "port": "5432", "database": "app",
                            "type": "dev", "auth-model": "native" }),
                ),
                saved("alice", "dbeaver-saved-canary-7741"),
            )
        })
        .collect();
    write_dbeaver(world, "General", "", &connections);
}

#[given(
    regex = r#"^un fichier de connexions qui déclare la première connexion DBeaver "([^"]+)" sous son nom suffixé$"#
)]
async fn file_with_first_suffixed(world: &mut ConnectionsWorld, name: String) {
    let dbeaver = world.context().catalog.clone();
    let discovery = dbeaver.discover();
    let first = discovery
        .connections
        .iter()
        .map(|entry| entry.name.clone())
        .find(|listed| listed.starts_with(&format!("{name} (")))
        .expect("a suffixed DBeaver duplicate");
    crate::fichier_de_connexions::write_connection_file(
        world,
        &format!(
            "[[connection]]\nname = \"{first}\"\nhost = \"db.example.com\"\nuser = \"alice\"\ntype = \"dev\"\nauth = \"command\"\npassword_command = [\"/bin/echo\", \"s3cret\"]\n"
        ),
    );
    world.probe = Some(json!(first));
}

pub async fn list(world: &mut ConnectionsWorld) {
    let context = world.context();
    let result = presentation::list_connections(&context.list_connections).await;
    world.listing = Some(serde_json::from_str(&text_of(&result)).expect("json listing"));
}

#[when(regex = r"^l'agent liste les connexions$")]
async fn agent_lists_connections(world: &mut ConnectionsWorld) {
    list(world).await;
}

#[given(regex = r"^l'agent a déjà listé les connexions$")]
async fn agent_listed_connections(world: &mut ConnectionsWorld) {
    list(world).await;
}

pub fn connections(world: &ConnectionsWorld) -> &Vec<Value> {
    world.listing.as_ref().expect("connections were listed")["connections"]
        .as_array()
        .expect("connections array")
}

pub fn entries<'a>(world: &'a ConnectionsWorld, name: &str) -> Vec<&'a Value> {
    connections(world)
        .iter()
        .filter(|connection| connection["name"] == name)
        .collect()
}

pub fn listed<'a>(world: &'a ConnectionsWorld, name: &str) -> &'a Value {
    entries(world, name)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("connection {name} is not listed in {:?}", world.listing))
}

fn hint(world: &ConnectionsWorld) -> String {
    world.listing.as_ref().expect("listed")["hint"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

fn reason(world: &ConnectionsWorld, name: &str) -> String {
    listed(world, name)["reason"]
        .as_str()
        .unwrap_or_else(|| panic!("{name} has no reason"))
        .to_string()
}

#[then(regex = r#"^la connexion "([^"]+)" est listée comme disponible$"#)]
async fn connection_is_available(world: &mut ConnectionsWorld, name: String) {
    let entry = listed(world, &name);
    assert_eq!(entry["status"], "available", "{entry}");
}

#[then(regex = r#"^la connexion "([^"]+)" est listée comme indisponible$"#)]
async fn connection_is_unavailable(world: &mut ConnectionsWorld, name: String) {
    let entry = listed(world, &name);
    assert_eq!(entry["status"], "unavailable", "{entry}");
}

#[then(regex = r#"^la connexion "([^"]+)" est listée comme non prise en charge$"#)]
async fn connection_is_unsupported(world: &mut ConnectionsWorld, name: String) {
    let entry = listed(world, &name);
    assert_eq!(entry["status"], "unsupported", "{entry}");
}

#[then(regex = r#"^la connexion "([^"]+)" est listée avec le statut "([^"]+)"$"#)]
async fn connection_has_status(world: &mut ConnectionsWorld, name: String, status: String) {
    let entry = listed(world, &name);
    assert_eq!(entry["status"], status, "{entry}");
}

#[then(
    regex = r#"^la connexion "([^"]+)" est listée avec (la source|le moteur|la base|le type) "([^"]+)"$"#
)]
async fn connection_has_field(
    world: &mut ConnectionsWorld,
    name: String,
    field: String,
    value: String,
) {
    let key = match field.as_str() {
        "la source" => "source",
        "le moteur" => "engine",
        "la base" => "database",
        _ => "type",
    };
    let entry = listed(world, &name);
    assert_eq!(entry[key], value, "{entry}");
}

#[then(regex = r#"^la connexion "([^"]+)" est listée sans base par défaut$"#)]
async fn connection_has_no_database(world: &mut ConnectionsWorld, name: String) {
    let entry = listed(world, &name);
    assert!(entry.get("database").is_none(), "{entry}");
}

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

#[then(regex = r#"^la connexion "([^"]+)" n'est listée qu'une seule fois$"#)]
async fn connection_listed_once(world: &mut ConnectionsWorld, name: String) {
    assert_eq!(entries(world, &name).len(), 1);
}

#[then(regex = r#"^la liste ne contient pas la connexion "([^"]+)"$"#)]
async fn connection_not_listed(world: &mut ConnectionsWorld, name: String) {
    assert!(entries(world, &name).is_empty(), "{name} is listed");
}

#[then(regex = r"^la liste ne contient aucune connexion$")]
async fn no_connection_listed(world: &mut ConnectionsWorld) {
    assert!(connections(world).is_empty(), "{:?}", world.listing);
}

#[then(regex = r#"^toutes les connexions nommées "([^"]+)" sont indisponibles$"#)]
async fn all_named_connections_unavailable(world: &mut ConnectionsWorld, name: String) {
    let found = entries(world, &name);
    assert_eq!(found.len(), 2, "both duplicates are listed");
    assert!(found.iter().all(|entry| entry["status"] == "unavailable"));
}

#[then(regex = r#"^la raison de la connexion "([^"]+)" contient "(.+)"$"#)]
async fn reason_contains(world: &mut ConnectionsWorld, name: String, expected: String) {
    let reason = reason(world, &name);
    assert!(reason.contains(&expected), "{reason}");
}

#[then(regex = r#"^la raison de la connexion "([^"]+)" ne contient pas "(.+)"$"#)]
async fn reason_does_not_contain(world: &mut ConnectionsWorld, name: String, forbidden: String) {
    let reason = reason(world, &name);
    assert!(!reason.contains(&forbidden), "{reason}");
}

#[then(regex = r"^la raison indique qu'aucun mot de passe n'est enregistré dans DBeaver$")]
async fn reason_names_missing_password(world: &mut ConnectionsWorld) {
    let reason = reason(world, CONNECTION);
    assert!(reason.contains("no saved password"), "{reason}");
}

#[then(
    regex = r#"^la liste ne montre ni l'hôte "([^"]+)", ni l'utilisateur "([^"]+)", ni le secret "([^"]+)"$"#
)]
async fn listing_hides(world: &mut ConnectionsWorld, host: String, user: String, secret: String) {
    let raw = world.listing.as_ref().expect("listed").to_string();
    for forbidden in [host, user, secret] {
        assert!(!raw.contains(&forbidden), "{forbidden} leaked in {raw}");
    }
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

fn count_from_source(world: &ConnectionsWorld, source: &str) -> usize {
    connections(world)
        .iter()
        .filter(|connection| connection["source"] == source)
        .count()
}

#[then(regex = r#"^aucune connexion de la source "([^"]+)" n'est listée$"#)]
async fn no_connection_from_source(world: &mut ConnectionsWorld, source: String) {
    assert_eq!(count_from_source(world, &source), 0, "{:?}", world.listing);
}

#[then(regex = r#"^une seule connexion de la source "([^"]+)" est listée$"#)]
async fn one_connection_from_source(world: &mut ConnectionsWorld, source: String) {
    assert_eq!(count_from_source(world, &source), 1, "{:?}", world.listing);
}

#[then(regex = r#"^le doublon déclaré dans le fichier est listé avec la source "([^"]+)"$"#)]
async fn declared_duplicate_source(world: &mut ConnectionsWorld, source: String) {
    let declared = world
        .probe
        .as_ref()
        .and_then(Value::as_str)
        .expect("a declared duplicate")
        .to_string();
    let found = entries(world, &declared);
    assert_eq!(found.len(), 1, "{declared}");
    assert_eq!(found[0]["source"], source);
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

#[then(
    regex = r#"^la vérification de clé d'hôte de la connexion "([^"]+)" est listée comme "([^"]+)"$"#
)]
async fn host_key_check_flag(world: &mut ConnectionsWorld, name: String, flag: String) {
    let entry = listed(world, &name);
    match flag.as_str() {
        "non signalé" => assert!(entry.get("ssh_host_key_check").is_none(), "{entry}"),
        value => assert_eq!(entry["ssh_host_key_check"], value, "{entry}"),
    }
}

#[then(regex = r"^l'indice cite la ligne (\d+)$")]
async fn hint_names_line(world: &mut ConnectionsWorld, line: usize) {
    let hint = hint(world);
    assert!(hint.contains(&format!("line {line},")), "{hint}");
}

#[then(regex = r#"^l'indice contient "(.+)"$"#)]
async fn hint_contains(world: &mut ConnectionsWorld, expected: String) {
    let hint = hint(world);
    assert!(hint.contains(&expected), "{hint}");
}

#[then(regex = r#"^l'indice ne contient pas "(.+)"$"#)]
async fn hint_does_not_contain(world: &mut ConnectionsWorld, forbidden: String) {
    let hint = hint(world);
    assert!(!hint.contains(&forbidden), "{hint}");
}

#[then(regex = r"^l'indice propose de créer le fichier ou de lancer l'import$")]
async fn hint_proposes_file_or_import(world: &mut ConnectionsWorld) {
    let hint = hint(world);
    assert!(hint.contains("connections.toml"), "{hint}");
    assert!(
        hint.contains("run `scarif-database-mcp connections import-dbeaver`"),
        "{hint}"
    );
}

#[then(regex = r"^l'indice cite le chemin du fichier de connexions$")]
async fn hint_names_file(world: &mut ConnectionsWorld) {
    let hint = hint(world);
    let path = world.connections_file().display().to_string();
    assert!(
        hint.contains(&format!(
            "SCARIF_CONNECTIONS_FILE points at {path}, which does not exist"
        )),
        "{hint}"
    );
}

#[then(regex = r"^la liste ne porte aucun indice$")]
async fn no_hint(world: &mut ConnectionsWorld) {
    let listing = world.listing.as_ref().expect("listed");
    assert!(listing.get("hint").is_none(), "{listing}");
}
