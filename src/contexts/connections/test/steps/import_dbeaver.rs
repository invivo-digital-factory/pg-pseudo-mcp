use std::os::unix::fs::PermissionsExt;

use cucumber::{given, then, when};
use scarif_database_mcp::contexts::connections::injector::import_dbeaver_use_case;
use scarif_database_mcp::contexts::connections::presentation::summary;
use serde_json::{Value, json};

use crate::bdd_support::{TunnelSpec, offline_target, write_workspace};
use crate::connections_world::ConnectionsWorld;
use crate::decouverte_des_connexions::{postgres_connection, write_dbeaver};

const EXISTING_CONTENT: &str = "# hand written, must survive\n";
const SAVED_SECRET: &str = "dbeaver-saved-canary-7741";

struct Spec<'a> {
    id: &'a str,
    name: &'a str,
    provider: &'a str,
    host: &'a str,
    auth_model: &'a str,
    connection_type: &'a str,
    tunnel: Option<(&'a str, &'a str)>,
    jump: bool,
}

fn postgres<'a>(id: &'a str, name: &'a str) -> Spec<'a> {
    Spec {
        id,
        name,
        provider: "postgresql",
        host: "db.internal.example",
        auth_model: "native",
        connection_type: "dev",
        tunnel: None,
        jump: false,
    }
}

fn write_connections(world: &ConnectionsWorld, specs: &[Spec<'_>]) {
    let connections: Vec<(String, Value, Value)> = specs
        .iter()
        .map(|spec| {
            let mut configuration = json!({
                "host": spec.host,
                "port": "5432",
                "database": "app",
                "type": spec.connection_type,
                "auth-model": spec.auth_model,
                "provider-properties": { "sslmode": "disable" }
            });
            let mut credentials =
                json!({ "#connection": { "user": "reader", "password": SAVED_SECRET } });
            if let Some((bastion, ssh_user)) = spec.tunnel {
                configuration["handlers"] = json!({
                    "ssh_tunnel": {
                        "type": "TUNNEL",
                        "enabled": true,
                        "properties": { "authType": "AGENT", "host": bastion, "port": 22 }
                    }
                });
                credentials["network/ssh_tunnel"] = json!({ "user": ssh_user });
            }
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
            let mut connection = postgres_connection(spec.name, configuration);
            connection["provider"] = json!(spec.provider);
            (spec.id.to_string(), connection, credentials)
        })
        .collect();
    write_dbeaver(world, "General", "", &connections);
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

#[given(regex = r#"^un espace de travail DBeaver avec une connexion "([^"]+)" de type "([^"]+)"$"#)]
async fn workspace_with_type(world: &mut ConnectionsWorld, name: String, connection_type: String) {
    write_connections(
        world,
        &[Spec {
            connection_type: &connection_type,
            ..postgres("pg-typed", &name)
        }],
    );
}

#[given(regex = r"^un espace de travail DBeaver avec une connexion MySQL seulement$")]
async fn workspace_with_mysql_only(world: &mut ConnectionsWorld) {
    write_connections(
        world,
        &[Spec {
            provider: "mysql",
            ..postgres("my-1", "legacy-mysql")
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
    regex = r#"^un espace de travail DBeaver avec deux connexions "([^"]+)" et "([^"]+)" derrière le bastion "([^"]+)"$"#
)]
async fn workspace_with_shared_bastion(
    world: &mut ConnectionsWorld,
    first: String,
    second: String,
    bastion: String,
) {
    write_connections(
        world,
        &[
            Spec {
                tunnel: Some((&bastion, "jump")),
                ..postgres("pg-a", &first)
            },
            Spec {
                tunnel: Some((&bastion, "jump")),
                ..postgres("pg-b", &second)
            },
        ],
    );
}

#[given(
    regex = r#"^un espace de travail DBeaver avec la connexion "([^"]+)" et la connexion "([^"]+)" derrière le bastion "([^"]+)" avec l'utilisateur SSH "([^"]+)"$"#
)]
async fn workspace_with_risky_tunnel(
    world: &mut ConnectionsWorld,
    kept: String,
    risky: String,
    bastion: String,
    user: String,
) {
    write_connections(
        world,
        &[
            postgres("pg-kept", &kept),
            Spec {
                tunnel: Some((&bastion, &user)),
                ..postgres("pg-risky", &risky)
            },
        ],
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

fn block_of(contents: &str, name: &str) -> String {
    let start = contents
        .find(&format!("name = \"{name}\""))
        .unwrap_or_else(|| panic!("{name} is not declared in {contents}"));
    let block = &contents[start..];
    block
        .split("[[connection]]")
        .next()
        .unwrap_or(block)
        .to_string()
}

fn value_of(contents: &str, name: &str, key: &str) -> String {
    let block = block_of(contents, name);
    block
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{key} = ")))
        .unwrap_or_else(|| panic!("no {key} for {name} in {block}"))
        .trim_matches('"')
        .to_string()
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

#[then(regex = r"^le fichier de connexions a les droits 600$")]
async fn file_is_private(world: &mut ConnectionsWorld) {
    let mode = std::fs::metadata(world.connections_file())
        .expect("connection file")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[then(regex = r"^l'import échoue en demandant l'option de remplacement$")]
async fn import_fails_asking_force(world: &mut ConnectionsWorld) {
    match world.import_outcome.as_ref().expect("an import was run") {
        Ok(_) => panic!("the import must fail"),
        Err(error) => assert!(error.contains("already exists; use --force"), "{error}"),
    }
}

#[then(regex = r#"^l'import échoue avec le message "(.+)"$"#)]
async fn import_fails_with(world: &mut ConnectionsWorld, expected: String) {
    match world.import_outcome.as_ref().expect("an import was run") {
        Ok(text) => panic!("the import must fail: {text}"),
        Err(error) => assert_eq!(error, &expected),
    }
}

#[then(regex = r"^aucun fichier de connexions n'a été écrit$")]
async fn nothing_written(world: &mut ConnectionsWorld) {
    assert!(!world.connections_file().exists());
}

#[then(regex = r"^le fichier de connexions existant est inchangé$")]
async fn existing_file_unchanged(world: &mut ConnectionsWorld) {
    assert_eq!(written(world), EXISTING_CONTENT);
}

#[then(regex = r#"^la connexion "([^"]+)" utilise l'authentification "([^"]+)"$"#)]
async fn connection_uses_auth(world: &mut ConnectionsWorld, name: String, auth: String) {
    assert_eq!(value_of(&written(world), &name, "auth"), auth);
}

#[then(regex = r#"^la connexion "([^"]+)" est importée avec le type "([^"]+)"$"#)]
async fn connection_has_type(world: &mut ConnectionsWorld, name: String, connection_type: String) {
    assert_eq!(value_of(&written(world), &name, "type"), connection_type);
}

#[then(regex = r#"^la connexion "([^"]+)" porte l'alias SSH "([^"]+)"$"#)]
async fn connection_has_alias(world: &mut ConnectionsWorld, name: String, alias: String) {
    assert_eq!(value_of(&written(world), &name, "ssh_alias"), alias);
}

#[then(regex = r"^le résumé demande une entrée pgpass ou une commande de mot de passe$")]
async fn summary_asks_for_pgpass_or_command(world: &mut ConnectionsWorld) {
    let text = imported_summary(world);
    assert!(text.contains("pgpass entry"), "{text}");
    assert!(text.contains("password_command"), "{text}");
}

#[then(regex = r"^le résumé annonce que le fichier prime désormais sur DBeaver$")]
async fn summary_announces_precedence(world: &mut ConnectionsWorld) {
    let text = imported_summary(world);
    let precedence = text
        .find("The connection file now takes precedence over DBeaver")
        .unwrap_or_else(|| panic!("{text}"));
    let per_connection = text
        .find("What is left to do")
        .unwrap_or_else(|| panic!("{text}"));
    assert!(precedence < per_connection, "{text}");
    assert!(text.contains("No password was written."), "{text}");
}

#[then(regex = r#"^le résumé contient "(.+)"$"#)]
async fn summary_contains(world: &mut ConnectionsWorld, expected: String) {
    let text = imported_summary(world);
    assert!(text.contains(&expected), "{text}");
}

#[then(regex = r#"^le résumé ne contient pas "(.+)"$"#)]
async fn summary_does_not_contain(world: &mut ConnectionsWorld, forbidden: String) {
    let text = imported_summary(world);
    assert!(!text.contains(&forbidden), "{text}");
}

fn host_blocks(text: &str) -> usize {
    text.lines()
        .filter(|line| line.starts_with("Host "))
        .count()
}

#[then(regex = r"^le résumé ne propose aucun bloc Host$")]
async fn summary_has_no_block(world: &mut ConnectionsWorld) {
    let text = imported_summary(world);
    assert_eq!(host_blocks(text), 0, "{text}");
    assert!(!text.contains("Suggested blocks"), "{text}");
}

#[then(regex = r"^le résumé propose exactement (\d+) bloc Host$")]
async fn summary_has_blocks(world: &mut ConnectionsWorld, count: usize) {
    let text = imported_summary(world);
    assert_eq!(host_blocks(text), count, "{text}");
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

#[then(regex = r#"^le résumé rappelle d'ajouter les clés d'hôte de "([^"]+)" à known_hosts$"#)]
async fn summary_reminds_known_hosts(world: &mut ConnectionsWorld, alias: String) {
    let text = imported_summary(world);
    let expected =
        format!("make sure the host keys of {alias} (and of any jump host) are in known_hosts");
    assert!(text.contains(&expected), "{text}");
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
