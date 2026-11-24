use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use scarif_database_mcp::contexts::connections::domain::types::ExecutionLimits;
use serde_json::{Value, json};

use crate::bdd_support::{database_target, offline_target};
use crate::connections_world::ConnectionsWorld;

fn expanded(world: &ConnectionsWorld, template: &str) -> String {
    let target = database_target_or_offline();
    template
        .replace("{dossier}", &world.config_dir.path().to_string_lossy())
        .replace("{base}", &target.database)
        .replace("{utilisateur}", &target.user)
        .replace("{mot_de_passe}", &target.password)
}

fn database_target_or_offline() -> crate::bdd_support::DatabaseTarget {
    if std::env::var(crate::bdd_support::DATABASE_URL_VAR).is_ok() {
        database_target()
    } else {
        offline_target()
    }
}

#[given(regex = r"^le fichier de connexions suivant :$")]
async fn connection_file(world: &mut ConnectionsWorld, step: &Step) {
    let template = step.docstring.as_deref().expect("a docstring");
    let contents = expanded(world, template.trim_start_matches('\n'));
    std::fs::write(world.connections_file(), contents).expect("connection file");
}

#[given(regex = r"^aucun fichier de connexions$")]
async fn no_connection_file(world: &mut ConnectionsWorld) {
    std::fs::remove_file(world.connections_file()).expect("remove connection file");
}

#[given(
    regex = r#"^un espace de travail DBeaver avec les connexions "([^"]+)" et "([^"]+)" dont les mots de passe sont enregistrés$"#
)]
async fn workspace_with_two_connections(
    world: &mut ConnectionsWorld,
    first: String,
    second: String,
) {
    let dir = world
        .workspace
        .path()
        .join("workspace6")
        .join("General")
        .join(".dbeaver");
    std::fs::create_dir_all(&dir).expect("workspace dir");
    let target = offline_target();
    let mut connections = serde_json::Map::new();
    let mut credentials = serde_json::Map::new();
    for name in [first, second] {
        let id = format!("postgres-jdbc-{name}");
        connections.insert(
            id.clone(),
            json!({
                "provider": "postgresql",
                "driver": "postgres-jdbc",
                "name": name,
                "save-password": true,
                "configuration": {
                    "host": target.host,
                    "port": target.port,
                    "database": target.database,
                    "type": "dev",
                    "auth-model": "native",
                    "provider-properties": { "sslmode": "disable" }
                }
            }),
        );
        credentials.insert(
            id,
            json!({ "#connection": { "user": target.user, "password": target.password } }),
        );
    }
    std::fs::write(
        dir.join("data-sources.json"),
        json!({ "folders": {}, "connections": connections }).to_string(),
    )
    .expect("data sources");
    std::fs::write(
        dir.join("credentials-config.json"),
        scarif_database_mcp::contexts::connections::infrastructure::data_sources::dbeaver::credentials::encrypt_for_tests(
            &Value::Object(credentials).to_string(),
            [7u8; 16],
        ),
    )
    .expect("credentials");
}

fn entries<'a>(world: &'a ConnectionsWorld, name: &str) -> Vec<&'a Value> {
    world.listing.as_ref().expect("connections were listed")["connections"]
        .as_array()
        .expect("connections array")
        .iter()
        .filter(|connection| connection["name"] == name)
        .collect()
}

fn hint(world: &ConnectionsWorld) -> String {
    world.listing.as_ref().expect("listed")["hint"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

#[then(regex = r#"^la connexion "([^"]+)" est listée avec la source "([^"]+)"$"#)]
async fn connection_has_source(world: &mut ConnectionsWorld, name: String, source: String) {
    let found = entries(world, &name);
    assert!(!found.is_empty(), "{name} is not listed");
    assert_eq!(found[0]["source"], source);
}

#[then(regex = r#"^la connexion "([^"]+)" est listée avec le moteur "([^"]+)"$"#)]
async fn connection_has_engine(world: &mut ConnectionsWorld, name: String, engine: String) {
    let found = entries(world, &name);
    assert!(!found.is_empty(), "{name} is not listed");
    assert_eq!(found[0]["engine"], engine);
}

#[then(regex = r#"^la connexion "([^"]+)" n'est listée qu'une seule fois$"#)]
async fn connection_listed_once(world: &mut ConnectionsWorld, name: String) {
    assert_eq!(entries(world, &name).len(), 1);
}

#[then(regex = r#"^la liste ne contient pas la connexion "([^"]+)"$"#)]
async fn connection_not_listed(world: &mut ConnectionsWorld, name: String) {
    assert!(entries(world, &name).is_empty(), "{name} is listed");
}

#[then(regex = r#"^la raison de la connexion "([^"]+)" contient "([^"]+)"$"#)]
async fn reason_contains(world: &mut ConnectionsWorld, name: String, expected: String) {
    let found = entries(world, &name);
    assert!(!found.is_empty(), "{name} is not listed");
    let reason = found[0]["reason"].as_str().expect("reason").to_string();
    assert!(reason.contains(&expected), "{reason}");
}

#[then(regex = r#"^la connexion "([^"]+)" est listée comme non prise en charge$"#)]
async fn connection_is_unsupported(world: &mut ConnectionsWorld, name: String) {
    let found = entries(world, &name);
    assert!(!found.is_empty(), "{name} is not listed");
    assert_eq!(found[0]["status"], "unsupported");
}

#[then(regex = r#"^toutes les connexions nommées "([^"]+)" sont indisponibles$"#)]
async fn all_named_connections_unavailable(world: &mut ConnectionsWorld, name: String) {
    let found = entries(world, &name);
    assert_eq!(found.len(), 2, "both duplicates are listed");
    assert!(found.iter().all(|entry| entry["status"] == "unavailable"));
}

#[then(regex = r"^l'indice cite la ligne (\d+)$")]
async fn hint_names_line(world: &mut ConnectionsWorld, line: usize) {
    let hint = hint(world);
    assert!(hint.contains(&format!("line {line}")), "{hint}");
}

#[then(regex = r#"^l'indice ne contient pas "([^"]+)"$"#)]
async fn hint_does_not_contain(world: &mut ConnectionsWorld, forbidden: String) {
    let hint = hint(world);
    assert!(!hint.contains(&forbidden), "{hint}");
}

#[then(regex = r"^l'indice propose de créer le fichier ou de lancer l'import$")]
async fn hint_proposes_file_or_import(world: &mut ConnectionsWorld) {
    let hint = hint(world);
    assert!(hint.contains("connections.toml"), "{hint}");
    assert!(hint.contains("connections import-dbeaver"), "{hint}");
}

#[then(regex = r"^la commande de mot de passe n'a pas été exécutée$")]
async fn command_was_not_run(world: &mut ConnectionsWorld) {
    assert!(!world.config_dir.path().join("marqueur").exists());
}

#[when(regex = r#"^l'agent exécute une requête de lecture sur la connexion "([^"]+)"$"#)]
async fn agent_runs_read_on(world: &mut ConnectionsWorld, name: String) {
    let context = world.context();
    let limits = ExecutionLimits {
        max_rows: 10,
        timeout_ms: 10_000,
    };
    world.outcome = Some(
        context
            .run_read_only_statement
            .execute(&name, None, "SELECT 1", limits)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string()),
    );
}

#[then(regex = r#"^la requête échoue avec une erreur qui contient "([^"]+)"$"#)]
async fn query_fails_with(world: &mut ConnectionsWorld, expected: String) {
    match &world.outcome {
        Some(Err(message)) => assert!(message.contains(&expected), "{message}"),
        other => panic!("the query should have failed: {other:?}"),
    }
}

#[then(regex = r#"^l'erreur de la requête ne contient pas "([^"]+)"$"#)]
async fn query_error_does_not_contain(world: &mut ConnectionsWorld, forbidden: String) {
    match &world.outcome {
        Some(Err(message)) => assert!(!message.contains(&forbidden), "{message}"),
        other => panic!("the query should have failed: {other:?}"),
    }
}
