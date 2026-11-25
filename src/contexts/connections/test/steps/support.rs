use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use cucumber::gherkin::Step;
use scarif_database_mcp::contexts::connections::domain::errors::ConnectionError;
use scarif_database_mcp::contexts::connections::domain::repositories::ConnectionCatalog;
use scarif_database_mcp::contexts::connections::domain::types::{SshHop, ExecutionLimits, ConnectParams, ConnectionEntry, Discovery};
use scarif_database_mcp::contexts::connections::infrastructure::data_sources::dbeaver::credentials::encrypt_for_tests;
use scarif_database_mcp::contexts::connections::presentation;
use serde_json::{json, Map, Value};

use crate::bdd_support::{
    BASTION_HOST, BASTION_USER, DATABASE_URL_VAR, DatabaseTarget, bastion_port, database_target,
    offline_target, public_key_fields, ssh_asset, text_of,
};
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

pub fn write_pgpass(world: &ConnectionsWorld, contents: &str, mode: u32) {
    let path = world.pgpass_file();
    std::fs::write(&path, contents).expect("pgpass file");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).expect("pgpass mode");
}

pub fn saved(user: &str, password: &str) -> Value {
    json!({ "#connection": { "user": user, "password": password } })
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

pub async fn list(world: &mut ConnectionsWorld) {
    let context = world.context();
    let result = presentation::list_connections(&context.list_connections).await;
    world.listing = Some(serde_json::from_str(&text_of(&result)).expect("json listing"));
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

pub fn hint(world: &ConnectionsWorld) -> String {
    world.listing.as_ref().expect("listed")["hint"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

pub fn reason(world: &ConnectionsWorld, name: &str) -> String {
    listed(world, name)["reason"]
        .as_str()
        .unwrap_or_else(|| panic!("{name} has no reason"))
        .to_string()
}

pub const LIMITS: ExecutionLimits = ExecutionLimits {
    max_rows: 10,
    timeout_ms: 10_000,
};

pub fn docstring(step: &Step) -> &str {
    step.docstring
        .as_deref()
        .expect("a docstring")
        .trim_start_matches('\n')
}

pub fn key_of(line: &str) -> &str {
    line.split('=').next().unwrap_or_default().trim()
}

pub fn record(world: &mut ConnectionsWorld, outcome: Result<(), ConnectionError>) {
    world.outcome = Some(outcome.clone().map_err(|error| error.to_string()));
    world.failure = outcome.err();
}

pub fn failure_message(world: &ConnectionsWorld) -> &str {
    match &world.outcome {
        Some(Err(message)) => message,
        other => panic!("the query should have failed: {other:?}"),
    }
}

pub fn discovery(world: &mut ConnectionsWorld) -> Discovery {
    match &world.discovery {
        Some(discovery) => discovery.clone(),
        None => world.context().catalog.discover(),
    }
}

pub fn entry(world: &mut ConnectionsWorld, name: &str) -> ConnectionEntry {
    discovery(world)
        .connections
        .into_iter()
        .find(|entry| entry.name == name)
        .unwrap_or_else(|| panic!("{name} is not discovered"))
}

pub fn params(world: &mut ConnectionsWorld, name: &str) -> ConnectParams {
    let entry = entry(world, name);
    entry
        .params
        .unwrap_or_else(|| panic!("{name} has no connection parameters: {:?}", entry.status))
}

pub fn assert_single_hop(hops: &[SshHop], endpoint: &str, user: &str) {
    let [hop] = hops else {
        panic!("one SSH hop is expected, got {hops:?}");
    };
    assert_eq!(format!("{}:{}", hop.host, hop.port), endpoint);
    assert_eq!(hop.user, user);
}

pub const STRICT_NO: &str = "  StrictHostKeyChecking no\n";

pub fn test_bastion(alias: &str, extra: &str) -> String {
    format!(
        "Host {alias}\n  HostName {BASTION_HOST}\n  Port {}\n  User {BASTION_USER}\n  IdentityFile {}\n  IdentitiesOnly yes\n  IdentityAgent none\n{extra}",
        bastion_port(),
        ssh_asset("tunnel_test_ed25519").display()
    )
}

pub fn bastion_key() -> String {
    public_key_fields("ssh_host_ed25519_test.pub")
}
