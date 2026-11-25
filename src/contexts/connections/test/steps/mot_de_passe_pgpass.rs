use std::os::unix::fs::PermissionsExt;

use cucumber::gherkin::Step;
use cucumber::{given, when};
use scarif_database_mcp::contexts::connections::domain::repositories::ConnectionCatalog;
use serde_json::{Value, json};

use crate::connections_steps::support::{
    postgres_connection, write_connection_file, write_dbeaver, write_pgpass, write_ssh_config,
};
use crate::connections_world::ConnectionsWorld;

#[given(regex = r"^un fichier pgpass aux droits (\d+) qui contient :$")]
async fn pgpass_file_with_mode(world: &mut ConnectionsWorld, mode: String, step: &Step) {
    let mode = u32::from_str_radix(&mode, 8).expect("octal mode");
    write_pgpass(world, step.docstring.as_deref().expect("a docstring"), mode);
}

#[given(regex = r"^un fichier pgpass remplacé par un dossier qui contient :$")]
async fn pgpass_directory(world: &mut ConnectionsWorld, step: &Step) {
    let dir = world.pgpass_file();
    std::fs::create_dir(&dir).expect("pgpass directory");
    std::fs::write(
        dir.join("pgpass"),
        step.docstring.as_deref().expect("a docstring"),
    )
    .expect("file inside the pgpass directory");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("dir mode");
}

#[given(
    regex = r#"^le fichier de connexions qui déclare la connexion "([^"]+)" vers la base "([^"]+)" en mode pgpass$"#
)]
async fn file_pgpass_connection(world: &mut ConnectionsWorld, name: String, database: String) {
    write_connection_file(
        world,
        &format!(
            "[[connection]]\nname = \"{name}\"\nhost = \"db.example.com\"\ndatabase = \"{database}\"\nuser = \"alice\"\ntype = \"dev\"\nauth = \"pgpass\"\n"
        ),
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

#[when(regex = r#"^le serveur prépare la connexion "([^"]+)" pour la base "([^"]+)"$"#)]
async fn prepare_for_database(world: &mut ConnectionsWorld, name: String, database: String) {
    let catalog = world.context().catalog.clone();
    let discovery = catalog.discover_in(Some(&database));
    let entry = discovery
        .connections
        .iter()
        .find(|entry| entry.name == name)
        .unwrap_or_else(|| panic!("{name} is not discovered"));
    assert_eq!(entry.database.as_deref(), Some(database.as_str()));
    world.discovery = Some(discovery);
}

#[given(
    regex = r#"^un fichier de configuration SSH qui définit l'alias "([^"]+)" vers "([^"]+)"$"#
)]
async fn ssh_config_alias_to(world: &mut ConnectionsWorld, alias: String, host: String) {
    write_ssh_config(world, &format!("Host {alias}\n  HostName {host}\n"));
}
