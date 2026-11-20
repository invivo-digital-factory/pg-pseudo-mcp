use std::path::{Path, PathBuf};

use pg_pseudo_mcp::contexts::connections::infrastructure::data_sources::dbeaver::credentials::encrypt_for_tests;
use serde_json::{Value, json};

pub const DATABASE_URL_VAR: &str = "PG_PSEUDO_TEST_DATABASE_URL";
pub const SSH_PORT_VAR: &str = "PG_PSEUDO_TEST_SSH_PORT";
pub const CONNECTION: &str = "canary-local";
pub const BASTION_HOST: &str = "localhost";
pub const BASTION_USER: &str = "tunnel";

pub struct DatabaseTarget {
    pub host: String,
    pub port: String,
    pub database: String,
    pub user: String,
    pub password: String,
}

pub struct TunnelSpec {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub key_path: PathBuf,
}

pub fn database_target() -> DatabaseTarget {
    let url = std::env::var(DATABASE_URL_VAR)
        .unwrap_or_else(|_| panic!("{DATABASE_URL_VAR} must point at the docker compose database"));
    let rest = url.strip_prefix("postgres://").expect("postgres:// url");
    let (credentials, location) = rest.split_once('@').expect("user:password@host");
    let (user, password) = credentials.split_once(':').expect("user:password");
    let (address, database) = location.split_once('/').expect("host:port/database");
    let (host, port) = address.split_once(':').expect("host:port");
    DatabaseTarget {
        host: host.to_string(),
        port: port.to_string(),
        database: database.to_string(),
        user: user.to_string(),
        password: password.to_string(),
    }
}

pub fn offline_target() -> DatabaseTarget {
    DatabaseTarget {
        host: "db.internal.example".to_string(),
        port: "5432".to_string(),
        database: "shop".to_string(),
        user: "reader".to_string(),
        password: "reader-secret".to_string(),
    }
}

pub fn text_of(result: &rmcp::model::CallToolResult) -> String {
    serde_json::to_value(result).expect("serializable result")["content"][0]["text"]
        .as_str()
        .expect("text content")
        .to_string()
}

pub fn is_error(result: &rmcp::model::CallToolResult) -> bool {
    result.is_error == Some(true)
}

pub fn bastion_port() -> u16 {
    std::env::var(SSH_PORT_VAR)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(55222)
}

pub fn ssh_asset(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("ssh")
        .join(name)
}

pub fn known_hosts_pattern() -> String {
    match bastion_port() {
        22 => BASTION_HOST.to_string(),
        port => format!("[{BASTION_HOST}]:{port}"),
    }
}

pub fn public_key_fields(name: &str) -> String {
    let line = std::fs::read_to_string(ssh_asset(name)).expect("public key file");
    let mut fields = line.split_whitespace();
    let algorithm = fields.next().expect("key algorithm");
    let key = fields.next().expect("key data");
    format!("{algorithm} {key}")
}

pub fn write_workspace(
    root: &Path,
    target: &DatabaseTarget,
    saved_password: bool,
    tunnel: Option<&TunnelSpec>,
) {
    let dir = root.join("workspace6").join("General").join(".dbeaver");
    std::fs::create_dir_all(&dir).expect("workspace dir");
    let mut configuration = json!({
        "host": target.host,
        "port": target.port,
        "database": target.database,
        "type": "dev",
        "auth-model": "native",
        "provider-properties": { "sslmode": "disable" }
    });
    if let Some(tunnel) = tunnel {
        configuration["handlers"] = json!({
            "ssh_tunnel": {
                "type": "TUNNEL",
                "enabled": true,
                "save-password": true,
                "properties": {
                    "authType": "PUBLIC_KEY",
                    "host": tunnel.host,
                    "port": tunnel.port,
                    "implementation": "sshj",
                    "jumpServer.count": 0,
                    "keyPath": tunnel.key_path.to_str().expect("utf-8 key path"),
                    "localHost": "",
                    "shareTunnels": true
                }
            }
        });
    }
    let sources = json!({
        "folders": {},
        "connections": {
            "postgres-jdbc-canary": {
                "provider": "postgresql",
                "driver": "postgres-jdbc",
                "name": CONNECTION,
                "save-password": saved_password,
                "configuration": configuration
            }
        }
    });
    std::fs::write(dir.join("data-sources.json"), sources.to_string()).expect("data sources");
    if !saved_password {
        return;
    }
    let mut entry = json!({
        "#connection": { "user": target.user, "password": target.password }
    });
    if let Some(tunnel) = tunnel {
        entry["network/ssh_tunnel"] = json!({ "user": tunnel.user });
    }
    let credentials: Value = json!({ "postgres-jdbc-canary": entry });
    std::fs::write(
        dir.join("credentials-config.json"),
        encrypt_for_tests(&credentials.to_string(), [7u8; 16]),
    )
    .expect("credentials");
}
