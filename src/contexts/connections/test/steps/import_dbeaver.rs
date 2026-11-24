use cucumber::{given, then, when};
use scarif_database_mcp::contexts::connections::injector::import_dbeaver_use_case;
use scarif_database_mcp::contexts::connections::presentation::summary;
use serde_json::{Map, Value, json};

use crate::bdd_support::{TunnelSpec, offline_target, write_workspace};
use crate::connections_world::ConnectionsWorld;

const EXISTING_CONTENT: &str = "# hand written, must survive\n";
const SAVED_SECRET: &str = "dbeaver-saved-canary-7741";

struct Spec<'a> {
    id: &'a str,
    name: &'a str,
    provider: &'a str,
    host: &'a str,
    auth_model: &'a str,
    jump: bool,
}

fn postgres<'a>(id: &'a str, name: &'a str) -> Spec<'a> {
    Spec {
        id,
        name,
        provider: "postgresql",
        host: "db.internal.example",
        auth_model: "native",
        jump: false,
    }
}

fn write_connections(world: &ConnectionsWorld, specs: &[Spec<'_>]) {
    let dir = world
        .workspace
        .path()
        .join("workspace6")
        .join("General")
        .join(".dbeaver");
    std::fs::create_dir_all(&dir).expect("workspace dir");
    let mut connections = Map::new();
    let mut credentials = Map::new();
    for spec in specs {
        let mut configuration = json!({
            "host": spec.host,
            "port": "5432",
            "database": "app",
            "type": "dev",
            "auth-model": spec.auth_model,
            "provider-properties": { "sslmode": "disable" }
        });
        if spec.jump {
            configuration["handlers"] = json!({
                "ssh_tunnel": {
                    "type": "TUNNEL",
                    "enabled": true,
                    "properties": {
                        "authType": "PUBLIC_KEY",
                        "host": "bastion.corp.example",
                        "port": 22,
                        "jumpServer.count": 1
                    }
                }
            });
        }
        connections.insert(
            spec.id.to_string(),
            json!({
                "provider": spec.provider,
                "driver": "driver",
                "name": spec.name,
                "save-password": true,
                "configuration": configuration
            }),
        );
        credentials.insert(
            spec.id.to_string(),
            json!({ "#connection": { "user": "reader", "password": SAVED_SECRET } }),
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

#[given(
    regex = r"^un espace de travail DBeaver avec une connexion à mot de passe natif enregistré$"
)]
async fn workspace_with_native_connection(world: &mut ConnectionsWorld) {
    let mut target = offline_target();
    target.password = SAVED_SECRET.to_string();
    write_workspace(world.workspace.path(), &target, true, None);
}

#[given(regex = r"^un espace de travail DBeaver avec une connexion pgpass sur un serveur Azure$")]
async fn workspace_with_azure_pgpass(world: &mut ConnectionsWorld) {
    write_connections(
        world,
        &[Spec {
            host: "shop.postgres.database.azure.com",
            auth_model: "postgres_pgpass",
            ..postgres("pg-azure", "azure-shop")
        }],
    );
}

#[given(regex = r"^un espace de travail DBeaver avec une connexion qui traverse un bastion SSH$")]
async fn workspace_with_tunnel(world: &mut ConnectionsWorld) {
    let tunnel = TunnelSpec {
        host: "bastion.corp.example".to_string(),
        port: 2222,
        user: "jump".to_string(),
        key_path: "/keys/id_ed25519".into(),
    };
    write_workspace(
        world.workspace.path(),
        &offline_target(),
        true,
        Some(&tunnel),
    );
}

#[given(
    regex = r"^un espace de travail DBeaver avec une connexion MySQL et une connexion à serveur de rebond$"
)]
async fn workspace_with_unsupported(world: &mut ConnectionsWorld) {
    write_connections(
        world,
        &[
            postgres("pg-ok", "kept"),
            Spec {
                provider: "mysql",
                ..postgres("my-1", "legacy-mysql")
            },
            Spec {
                jump: true,
                ..postgres("pg-jump", "jumped")
            },
        ],
    );
}

#[given(regex = r#"^un espace de travail DBeaver avec deux connexions nommées "([^"]+)"$"#)]
async fn workspace_with_duplicates(world: &mut ConnectionsWorld, name: String) {
    write_connections(world, &[postgres("pg-1", &name), postgres("pg-2", &name)]);
}

#[given(regex = r"^un fichier de connexions existant$")]
async fn existing_connection_file(world: &mut ConnectionsWorld) {
    std::fs::write(world.connections_file(), EXISTING_CONTENT).expect("existing file");
}

fn run_import(world: &mut ConnectionsWorld, force: bool) {
    let file = world.connections_file();
    if std::fs::metadata(&file).is_ok_and(|metadata| metadata.len() == 0) {
        std::fs::remove_file(&file).expect("remove the empty placeholder");
    }
    let use_case = import_dbeaver_use_case(&world.config());
    world.import_outcome = Some(
        use_case
            .execute(force)
            .map(|report| summary(&report))
            .map_err(|error| error.to_string()),
    );
}

#[when(regex = r"^l'utilisateur lance l'import des connexions DBeaver$")]
async fn user_runs_import(world: &mut ConnectionsWorld) {
    run_import(world, false);
}

#[when(regex = r"^l'utilisateur lance l'import des connexions DBeaver avec remplacement$")]
async fn user_runs_import_with_force(world: &mut ConnectionsWorld) {
    run_import(world, true);
}

fn written(world: &ConnectionsWorld) -> String {
    std::fs::read_to_string(world.connections_file()).expect("connection file")
}

fn imported_summary(world: &ConnectionsWorld) -> &str {
    match world.import_outcome.as_ref().expect("an import was run") {
        Ok(text) => text,
        Err(error) => panic!("the import failed: {error}"),
    }
}

#[then(regex = r#"^le fichier de connexions déclare la connexion "([^"]+)"$"#)]
async fn file_declares(world: &mut ConnectionsWorld, name: String) {
    let contents = written(world);
    assert!(
        contents.contains(&format!("name = \"{name}\"")),
        "{contents}"
    );
}

#[then(
    regex = r"^le fichier de connexions ne contient pas le mot de passe enregistré dans DBeaver$"
)]
async fn file_has_no_saved_secret(world: &mut ConnectionsWorld) {
    let contents = written(world);
    assert!(!contents.contains(SAVED_SECRET), "{contents}");
    assert!(!imported_summary(world).contains(SAVED_SECRET));
}

#[then(regex = r"^l'import échoue en demandant l'option de remplacement$")]
async fn import_fails_asking_force(world: &mut ConnectionsWorld) {
    match world.import_outcome.as_ref().expect("an import was run") {
        Ok(_) => panic!("the import must fail"),
        Err(error) => assert!(error.contains("--force"), "{error}"),
    }
}

#[then(regex = r"^le fichier de connexions existant est inchangé$")]
async fn existing_file_unchanged(world: &mut ConnectionsWorld) {
    assert_eq!(written(world), EXISTING_CONTENT);
}

fn auth_of(contents: &str, name: &str) -> String {
    let start = contents
        .find(&format!("name = \"{name}\""))
        .unwrap_or_else(|| panic!("{name} is not declared in {contents}"));
    let block = &contents[start..];
    let block = block.split("[[connection]]").next().unwrap_or(block);
    block
        .lines()
        .find_map(|line| line.strip_prefix("auth = "))
        .unwrap_or_else(|| panic!("no auth for {name} in {block}"))
        .trim_matches('"')
        .to_string()
}

#[then(regex = r#"^la connexion "([^"]+)" utilise l'authentification "([^"]+)"$"#)]
async fn connection_uses_auth(world: &mut ConnectionsWorld, name: String, auth: String) {
    assert_eq!(auth_of(&written(world), &name), auth);
}

#[then(regex = r"^le résumé demande une entrée pgpass ou une commande de mot de passe$")]
async fn summary_asks_for_pgpass_or_command(world: &mut ConnectionsWorld) {
    let text = imported_summary(world);
    assert!(text.contains("pgpass entry"), "{text}");
    assert!(text.contains("password_command"), "{text}");
}

#[then(regex = r#"^la connexion "([^"]+)" porte l'alias SSH "([^"]+)"$"#)]
async fn connection_has_alias(world: &mut ConnectionsWorld, name: String, alias: String) {
    let contents = written(world);
    let start = contents
        .find(&format!("name = \"{name}\""))
        .expect("connection declared");
    assert!(
        contents[start..].contains(&format!("ssh_alias = \"{alias}\"")),
        "{contents}"
    );
}

#[then(
    regex = r"^le résumé propose un bloc Host avec l'hôte, le port, l'utilisateur et le fichier de clé$"
)]
async fn summary_suggests_host_block(world: &mut ConnectionsWorld) {
    let text = imported_summary(world);
    for expected in [
        "Host bastion.corp.example",
        "HostName bastion.corp.example",
        "Port 2222",
        "User jump",
        "IdentityFile /keys/id_ed25519",
    ] {
        assert!(text.contains(expected), "{expected} missing from {text}");
    }
}

#[then(regex = r#"^le résumé ignore la connexion "([^"]+)" avec la raison "([^"]+)"$"#)]
async fn summary_skips(world: &mut ConnectionsWorld, name: String, reason: String) {
    let text = imported_summary(world);
    let line = text
        .lines()
        .find(|line| line.trim_start().starts_with(&format!("{name}:")))
        .unwrap_or_else(|| panic!("{name} is not reported as skipped in {text}"));
    assert!(line.contains(&reason), "{line}");
}

#[then(regex = r#"^chaque nom de connexion est listé une seule fois avec la source "([^"]+)"$"#)]
async fn every_name_once_with_source(world: &mut ConnectionsWorld, source: String) {
    let listing = world.listing.as_ref().expect("listed");
    let connections = listing["connections"].as_array().expect("connections");
    assert_eq!(connections.len(), 2, "{listing}");
    let mut names: Vec<&str> = connections
        .iter()
        .map(|connection| connection["name"].as_str().expect("name"))
        .collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 2, "{listing}");
    assert!(
        connections
            .iter()
            .all(|connection| connection["source"] == source)
    );
}

#[then(
    regex = r#"^le fichier de connexions ne déclare aucune connexion nommée exactement "([^"]+)"$"#
)]
async fn no_exact_name(world: &mut ConnectionsWorld, name: String) {
    let contents = written(world);
    assert!(
        !contents.contains(&format!("name = \"{name}\"")),
        "{contents}"
    );
}
