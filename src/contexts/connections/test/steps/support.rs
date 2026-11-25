use std::path::{Path, PathBuf};

use scarif_database_mcp::contexts::connections::infrastructure::data_sources::dbeaver::credentials::encrypt_for_tests;
use serde_json::{Map, Value, json};

use crate::bdd_support::{DATABASE_URL_VAR, DatabaseTarget, database_target, offline_target};
use crate::connections_world::ConnectionsWorld;

pub fn database_target_or_offline() -> DatabaseTarget {
    if std::env::var(DATABASE_URL_VAR).is_ok() {
        database_target()
    } else {
        offline_target()
    }
}

pub fn expanded(world: &ConnectionsWorld, template: &str) -> String {
    let target = database_target_or_offline();
    template
        .replace("{dossier}", &world.config_dir.path().to_string_lossy())
        .replace("{base}", &target.database)
        .replace("{utilisateur}", &target.user)
        .replace("{mot_de_passe}", &target.password)
}

pub fn write_connection_file(world: &ConnectionsWorld, contents: &str) {
    let file = world.connections_file();
    if file.is_dir() {
        std::fs::remove_dir(&file).expect("remove the directory in place of the file");
    }
    std::fs::write(file, expanded(world, contents)).expect("connection file");
}

pub fn write_ssh_config(world: &mut ConnectionsWorld, contents: &str) {
    let path = world.config_dir.path().join("ssh_config");
    std::fs::write(&path, contents).expect("ssh config");
    world.ssh_config = Some(path);
}

pub fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("executable script");
}

pub fn dbeaver_dir(world: &ConnectionsWorld, project: &str) -> PathBuf {
    world
        .workspace
        .path()
        .join("workspace6")
        .join(project)
        .join(".dbeaver")
}

pub fn write_dbeaver(
    world: &ConnectionsWorld,
    project: &str,
    suffix: &str,
    connections: &[(String, Value, Value)],
) {
    let dir = dbeaver_dir(world, project);
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

pub fn write_credentials(
    world: &ConnectionsWorld,
    project: &str,
    suffix: &str,
    credentials: &Value,
) {
    std::fs::write(
        dbeaver_dir(world, project).join(format!("credentials-config{suffix}.json")),
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
