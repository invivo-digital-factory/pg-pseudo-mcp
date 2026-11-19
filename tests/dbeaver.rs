use std::fs;
use std::path::Path;

use pg_pseudo_mcp::dbeaver::credentials::encrypt_for_tests;
use pg_pseudo_mcp::dbeaver::{
    ConnectionEntry, ConnectionStatus, SslMode, discover, find_connection,
};
use pg_pseudo_mcp::state::AppConfig;
use pg_pseudo_mcp::tools::connections::list_connections;
use pg_pseudo_mcp::tools::errors::ErrorCategory;
use serde_json::{Value, json};
use tempfile::TempDir;

fn write_project(root: &Path, project: &str, suffix: &str, sources: &Value, creds: Option<&Value>) {
    let dir = root.join("workspace6").join(project).join(".dbeaver");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(format!("data-sources{suffix}.json")),
        serde_json::to_vec_pretty(sources).unwrap(),
    )
    .unwrap();
    if let Some(creds) = creds {
        let iv: [u8; 16] = rand::random();
        fs::write(
            dir.join(format!("credentials-config{suffix}.json")),
            encrypt_for_tests(&creds.to_string(), iv),
        )
        .unwrap();
    }
}

fn pg_connection(name: &str, extra: Value) -> Value {
    let mut connection = json!({
        "provider": "postgresql",
        "driver": "postgres-jdbc",
        "name": name,
        "save-password": true,
        "configuration": {
            "host": "db.internal.example",
            "port": "5433",
            "database": "shop",
            "type": "dev",
            "auth-model": "native"
        }
    });
    merge(&mut connection, extra);
    connection
}

fn merge(target: &mut Value, patch: Value) {
    match (target, patch) {
        (Value::Object(target), Value::Object(patch)) => {
            for (key, value) in patch {
                merge(target.entry(key).or_insert(Value::Null), value);
            }
        }
        (target, patch) => *target = patch,
    }
}

fn sources(connections: Value) -> Value {
    json!({ "folders": {}, "connection-types": {}, "connections": connections })
}

fn creds(id: &str, user: &str, password: &str) -> Value {
    json!({ id: { "#connection": { "user": user, "password": password } } })
}

fn config(root: &Path) -> AppConfig {
    AppConfig {
        dbeaver_home: Some(root.to_path_buf()),
        data_dir: None,
    }
}

fn only(root: &Path) -> ConnectionEntry {
    let mut discovery = discover(Some(root));
    assert_eq!(discovery.connections.len(), 1, "{discovery:?}");
    discovery.connections.remove(0)
}

fn reason(status: &ConnectionStatus) -> &str {
    match status {
        ConnectionStatus::Unavailable { reason } | ConnectionStatus::Unsupported { reason } => {
            reason
        }
        ConnectionStatus::Available => panic!("expected a reason"),
    }
}

#[test]
fn encrypted_credentials_yield_an_available_connection() {
    let home = TempDir::new().unwrap();
    write_project(
        home.path(),
        "General",
        "",
        &sources(json!({ "pg-1": pg_connection("Shop", json!({})) })),
        Some(&creds("pg-1", "reader", "s3cret-pw")),
    );
    let entry = only(home.path());
    assert_eq!(entry.name, "Shop");
    assert_eq!(entry.project, "General");
    assert_eq!(entry.engine, "postgresql");
    assert_eq!(entry.connection_type, "dev");
    assert_eq!(entry.database.as_deref(), Some("shop"));
    assert_eq!(entry.status, ConnectionStatus::Available);
    let params = entry.params.unwrap();
    assert_eq!(params.host, "db.internal.example");
    assert_eq!(params.port, 5433);
    assert_eq!(params.database, "shop");
    assert_eq!(params.user, "reader");
    assert_eq!(params.password.expose(), "s3cret-pw");
    assert_eq!(params.sslmode, SslMode::Prefer);
}

#[test]
fn secondary_data_sources_file_is_listed_alongside_the_default() {
    let home = TempDir::new().unwrap();
    write_project(
        home.path(),
        "General",
        "",
        &sources(json!({ "pg-1": pg_connection("Shop", json!({})) })),
        Some(&creds("pg-1", "a", "b")),
    );
    write_project(
        home.path(),
        "Second",
        "-2",
        &sources(json!({ "pg-2": pg_connection("Billing", json!({})) })),
        Some(&creds("pg-2", "c", "d")),
    );
    let discovery = discover(Some(home.path()));
    let mut names: Vec<_> = discovery
        .connections
        .iter()
        .map(|entry| (entry.name.as_str(), entry.status.clone()))
        .collect();
    names.sort_by(|a, b| a.0.cmp(b.0));
    assert_eq!(
        names,
        vec![
            ("Billing", ConnectionStatus::Available),
            ("Shop", ConnectionStatus::Available)
        ]
    );
}

#[test]
fn plain_user_and_password_override_the_credentials_file() {
    let home = TempDir::new().unwrap();
    write_project(
        home.path(),
        "General",
        "",
        &sources(json!({ "pg-1": pg_connection("Shop", json!({
            "configuration": { "user": "plain-user", "password": "plain-pw" }
        })) })),
        Some(&creds("pg-1", "file-user", "file-pw")),
    );
    let params = only(home.path()).params.unwrap();
    assert_eq!(params.user, "plain-user");
    assert_eq!(params.password.expose(), "plain-pw");
}

#[test]
fn saved_password_without_credentials_entry_is_unavailable() {
    let home = TempDir::new().unwrap();
    write_project(
        home.path(),
        "General",
        "",
        &sources(json!({ "pg-1": pg_connection("Shop", json!({})) })),
        Some(&json!({})),
    );
    let entry = only(home.path());
    assert!(matches!(entry.status, ConnectionStatus::Unavailable { .. }));
    let text = reason(&entry.status);
    assert!(text.contains("master password"), "{text}");
    assert!(text.contains("secure storage"), "{text}");
    assert!(entry.params.is_none());
}

#[test]
fn password_not_saved_is_unavailable() {
    let home = TempDir::new().unwrap();
    write_project(
        home.path(),
        "General",
        "",
        &sources(json!({ "pg-1": pg_connection("Shop", json!({ "save-password": false })) })),
        None,
    );
    let entry = only(home.path());
    assert!(matches!(entry.status, ConnectionStatus::Unavailable { .. }));
    assert!(reason(&entry.status).contains("no saved password"));
}

#[test]
fn invalid_json_data_sources_is_unavailable_and_other_projects_still_load() {
    let home = TempDir::new().unwrap();
    let broken = home.path().join("workspace6/Broken/.dbeaver");
    fs::create_dir_all(&broken).unwrap();
    fs::write(broken.join("data-sources.json"), [0x8f, 0x01, 0xfe, 0x42]).unwrap();
    write_project(
        home.path(),
        "General",
        "",
        &sources(json!({ "pg-1": pg_connection("Shop", json!({})) })),
        Some(&creds("pg-1", "a", "b")),
    );
    let discovery = discover(Some(home.path()));
    assert_eq!(discovery.connections.len(), 2);
    let broken_entry = discovery
        .connections
        .iter()
        .find(|entry| entry.project == "Broken")
        .unwrap();
    assert!(matches!(
        broken_entry.status,
        ConnectionStatus::Unavailable { .. }
    ));
    assert!(!reason(&broken_entry.status).is_empty());
    assert!(
        discovery
            .connections
            .iter()
            .any(|entry| entry.name == "Shop" && entry.status == ConnectionStatus::Available)
    );
}

#[test]
fn enabled_ssh_tunnel_is_unsupported() {
    let home = TempDir::new().unwrap();
    write_project(
        home.path(),
        "General",
        "",
        &sources(json!({ "pg-1": pg_connection("Shop", json!({
            "configuration": { "handlers": { "ssh_tunnel": { "type": "TUNNEL", "enabled": true } } }
        })) })),
        Some(&creds("pg-1", "a", "b")),
    );
    let entry = only(home.path());
    assert!(matches!(entry.status, ConnectionStatus::Unsupported { .. }));
    assert!(reason(&entry.status).contains("SSH"));
}

#[test]
fn disabled_ssh_tunnel_is_ignored() {
    let home = TempDir::new().unwrap();
    write_project(
        home.path(),
        "General",
        "",
        &sources(json!({ "pg-1": pg_connection("Shop", json!({
            "configuration": { "handlers": { "ssh_tunnel": { "type": "TUNNEL", "enabled": false } } }
        })) })),
        Some(&creds("pg-1", "a", "b")),
    );
    assert_eq!(only(home.path()).status, ConnectionStatus::Available);
}

#[test]
fn other_engine_is_unsupported_and_names_the_engine() {
    let home = TempDir::new().unwrap();
    write_project(
        home.path(),
        "General",
        "",
        &sources(json!({ "my-1": pg_connection("Legacy", json!({ "provider": "mysql" })) })),
        None,
    );
    let entry = only(home.path());
    assert!(matches!(entry.status, ConnectionStatus::Unsupported { .. }));
    assert!(reason(&entry.status).contains("mysql"));
    assert_eq!(entry.engine, "mysql");
}

#[test]
fn non_native_auth_model_is_unsupported() {
    let home = TempDir::new().unwrap();
    write_project(
        home.path(),
        "General",
        "",
        &sources(json!({ "pg-1": pg_connection("Shop", json!({
            "configuration": { "auth-model": "postgres_pgpass" }
        })) })),
        Some(&creds("pg-1", "a", "b")),
    );
    assert!(matches!(
        only(home.path()).status,
        ConnectionStatus::Unsupported { .. }
    ));
}

#[test]
fn variable_resolves_from_environment_when_set() {
    let home = TempDir::new().unwrap();
    write_project(
        home.path(),
        "General",
        "",
        &sources(json!({ "pg-1": pg_connection("Shop", json!({
            "configuration": { "host": "${PGMCP_TEST_HOST_SET:localhost}" }
        })) })),
        Some(&creds("pg-1", "a", "b")),
    );
    unsafe { std::env::set_var("PGMCP_TEST_HOST_SET", "env-host.example") };
    let params = only(home.path()).params.unwrap();
    assert_eq!(params.host, "env-host.example");
}

#[test]
fn variable_falls_back_to_its_default() {
    let home = TempDir::new().unwrap();
    write_project(
        home.path(),
        "General",
        "",
        &sources(json!({ "pg-1": pg_connection("Shop", json!({
            "configuration": { "host": "${PGMCP_TEST_HOST_UNSET:localhost}" }
        })) })),
        Some(&creds("pg-1", "a", "b")),
    );
    assert_eq!(only(home.path()).params.unwrap().host, "localhost");
}

#[test]
fn unresolved_variable_without_default_is_unavailable() {
    let home = TempDir::new().unwrap();
    write_project(
        home.path(),
        "General",
        "",
        &sources(json!({ "pg-1": pg_connection("Shop", json!({
            "configuration": { "host": "${PGMCP_TEST_HOST_NEVER}" }
        })) })),
        Some(&creds("pg-1", "a", "b")),
    );
    let entry = only(home.path());
    assert!(matches!(entry.status, ConnectionStatus::Unavailable { .. }));
    assert!(reason(&entry.status).contains("PGMCP_TEST_HOST_NEVER"));
}

#[test]
fn url_mode_parses_host_port_database_and_sslmode() {
    let home = TempDir::new().unwrap();
    write_project(
        home.path(),
        "General",
        "",
        &sources(json!({ "pg-1": pg_connection("Shop", json!({
            "configuration": {
                "configurationType": "URL",
                "host": null,
                "port": null,
                "database": null,
                "url": "jdbc:postgresql://url-host.example:6543/urldb?sslmode=require&ApplicationName=x"
            }
        })) })),
        Some(&creds("pg-1", "a", "b")),
    );
    let params = only(home.path()).params.unwrap();
    assert_eq!(params.host, "url-host.example");
    assert_eq!(params.port, 6543);
    assert_eq!(params.database, "urldb");
    assert_eq!(params.sslmode, SslMode::Require);
}

#[test]
fn sslmode_is_read_from_provider_properties() {
    let home = TempDir::new().unwrap();
    write_project(
        home.path(),
        "General",
        "",
        &sources(json!({ "pg-1": pg_connection("Shop", json!({
            "configuration": { "provider-properties": { "sslmode": "verify-full" } }
        })) })),
        Some(&creds("pg-1", "a", "b")),
    );
    assert_eq!(
        only(home.path()).params.unwrap().sslmode,
        SslMode::VerifyFull
    );
}

#[test]
fn debug_of_params_never_prints_the_password() {
    let home = TempDir::new().unwrap();
    write_project(
        home.path(),
        "General",
        "",
        &sources(json!({ "pg-1": pg_connection("Shop", json!({})) })),
        Some(&creds("pg-1", "reader", "hunter2-canary")),
    );
    let entry = only(home.path());
    let params = entry.params.clone().unwrap();
    assert!(!format!("{params:?}").contains("hunter2-canary"));
    assert!(!format!("{entry:?}").contains("hunter2-canary"));
}

#[test]
fn missing_dbeaver_directory_returns_an_empty_list_with_a_hint() {
    let home = TempDir::new().unwrap();
    let missing = home.path().join("nowhere");
    let discovery = discover(Some(&missing));
    assert!(discovery.connections.is_empty());
    assert!(discovery.hint.is_some());
}

#[test]
fn duplicate_names_get_a_stable_suffix() {
    let home = TempDir::new().unwrap();
    for project in ["A", "B"] {
        write_project(
            home.path(),
            project,
            "",
            &sources(json!({ "pg-1": pg_connection("Shop", json!({})) })),
            Some(&creds("pg-1", "a", "b")),
        );
    }
    let first: Vec<String> = discover(Some(home.path()))
        .connections
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    let second: Vec<String> = discover(Some(home.path()))
        .connections
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    assert_eq!(first, second);
    assert_eq!(first.len(), 2);
    assert_ne!(first[0], first[1]);
    assert!(first.iter().all(|name| name.starts_with("Shop (")));
}

#[test]
fn find_connection_reports_unknown_and_unavailable() {
    let home = TempDir::new().unwrap();
    write_project(
        home.path(),
        "General",
        "",
        &sources(json!({
            "pg-1": pg_connection("Shop", json!({})),
            "pg-2": pg_connection("Locked", json!({ "save-password": false }))
        })),
        Some(&creds("pg-1", "a", "b")),
    );
    let config = config(home.path());

    let unknown = find_connection(&config, "Nope").unwrap_err();
    assert_eq!(unknown.code, "unknown_connection");
    assert_eq!(unknown.category, ErrorCategory::UnknownConnection);
    assert!(unknown.hint.unwrap().contains("Shop"));

    let unavailable = find_connection(&config, "Locked").unwrap_err();
    assert_eq!(unavailable.code, "connection_unavailable");
    assert_eq!(unavailable.category, ErrorCategory::ConnectionUnavailable);
    assert!(unavailable.message.contains("no saved password"));
    assert_eq!(
        unavailable.hint.as_deref(),
        Some("ask the developer to fix the connection in DBeaver")
    );

    assert_eq!(find_connection(&config, "Shop").unwrap().user, "a");
}

#[test]
fn find_connection_rereads_the_workspace_on_every_call() {
    let home = TempDir::new().unwrap();
    let config = config(home.path());
    assert!(find_connection(&config, "Shop").is_err());
    write_project(
        home.path(),
        "General",
        "",
        &sources(json!({ "pg-1": pg_connection("Shop", json!({})) })),
        Some(&creds("pg-1", "a", "b")),
    );
    assert!(find_connection(&config, "Shop").is_ok());
}

#[tokio::test]
async fn list_connections_output_has_no_host_user_or_password() {
    let home = TempDir::new().unwrap();
    write_project(
        home.path(),
        "General",
        "",
        &sources(json!({
            "pg-1": pg_connection("Shop", json!({ "folder": "Prod" })),
            "my-1": pg_connection("Legacy", json!({ "provider": "mysql" }))
        })),
        Some(&creds("pg-1", "canary-user", "canary-password")),
    );
    let result = list_connections(&config(home.path())).await;
    let text = serde_json::to_string(&result).unwrap();
    for forbidden in [
        "db.internal.example",
        "canary-user",
        "canary-password",
        "5433",
    ] {
        assert!(!text.contains(forbidden), "{forbidden} leaked in {text}");
    }
    let body: Value = serde_json::from_str(
        result.content[0]
            .as_text()
            .map(|text| text.text.as_str())
            .unwrap(),
    )
    .unwrap();
    let connections = body["connections"].as_array().unwrap();
    assert_eq!(connections.len(), 2);
    let shop = connections.iter().find(|c| c["name"] == "Shop").unwrap();
    assert_eq!(shop["status"], "available");
    assert_eq!(shop["engine"], "postgresql");
    assert_eq!(shop["type"], "dev");
    assert_eq!(shop["database"], "shop");
    assert_eq!(shop["folder"], "Prod");
    assert!(shop.get("reason").is_none());
    let legacy = connections.iter().find(|c| c["name"] == "Legacy").unwrap();
    assert_eq!(legacy["status"], "unsupported");
    assert!(legacy["reason"].as_str().unwrap().contains("mysql"));
}
