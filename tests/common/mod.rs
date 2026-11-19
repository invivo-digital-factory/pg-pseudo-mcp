#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;

use base64::Engine;
use md5::{Digest, Md5};
use pg_pseudo_mcp::dbeaver::credentials::encrypt_for_tests;
use pg_pseudo_mcp::pseudo::Pseudonymizer;
use pg_pseudo_mcp::state::{AppConfig, AppState};
use rmcp::model::CallToolResult;
use serde_json::{Value, json};

pub const DATABASE_URL_VAR: &str = "PG_PSEUDO_TEST_DATABASE_URL";
pub const CONNECTION: &str = "canary-local";

pub const CANARY_VALUES: &[&str] = &[
    "Ilona",
    "Kervadec",
    "ilona.kervadec@canary-mail.fr",
    "06 41 27 83 95",
    "33641278395",
    "FR76 3000 4000 0312 3456 7890 143",
    "418273652",
    "2851175123456 22",
    "o.thaumassin@canary-mail.fr",
    "kervadec.ilona@canary-mail.fr",
    "ilona.k@canary-mail.fr",
    "ik.pro@canary-mail.fr",
    "Odilon",
    "Thaumassin",
    "odilon.thaumassin@canary-mail.fr",
    "07 58 14 92 36",
    "33758149236",
    "FR14 2004 1010 0505 0001 3M02 606",
    "odilon.t@canary-mail.fr",
    "Maëlys",
    "Brizolier",
    "maelys.brizolier@canary-mail.fr",
    "Ilona Kervadec",
    "Odilon Thaumassin",
    "06 52 18 74 30",
    "12 rue des Lilas",
];

pub fn database_url() -> String {
    std::env::var(DATABASE_URL_VAR)
        .unwrap_or_else(|_| panic!("{DATABASE_URL_VAR} must point at the docker compose database"))
}

pub struct Harness {
    pub state: Arc<AppState>,
    _dbeaver: tempfile::TempDir,
    _data: tempfile::TempDir,
}

impl Harness {
    pub fn new() -> Self {
        let url = database_url();
        let rest = url.strip_prefix("postgres://").expect("postgres:// url");
        let (credentials, location) = rest.split_once('@').expect("user:password@host");
        let (user, password) = credentials.split_once(':').expect("user:password");
        let (address, database) = location.split_once('/').expect("host:port/database");
        let (host, port) = address.split_once(':').expect("host:port");
        let dbeaver = tempfile::tempdir().expect("dbeaver dir");
        write_workspace(dbeaver.path(), host, port, database, user, password);
        let data = tempfile::tempdir().expect("data dir");
        let config = AppConfig {
            dbeaver_home: Some(dbeaver.path().to_path_buf()),
            data_dir: Some(data.path().to_path_buf()),
        };
        let pseudonymizer = Pseudonymizer::open(data.path())
            .map(Arc::new)
            .map_err(|error| error.to_string());
        Self {
            state: Arc::new(AppState::with_pseudonymizer(config, pseudonymizer)),
            _dbeaver: dbeaver,
            _data: data,
        }
    }
}

fn write_workspace(
    root: &Path,
    host: &str,
    port: &str,
    database: &str,
    user: &str,
    password: &str,
) {
    let dir = root.join("workspace6").join("General").join(".dbeaver");
    std::fs::create_dir_all(&dir).expect("workspace dir");
    let sources = json!({
        "folders": {},
        "connections": {
            "postgres-jdbc-canary": {
                "provider": "postgresql",
                "driver": "postgres-jdbc",
                "name": CONNECTION,
                "save-password": true,
                "configuration": {
                    "host": host,
                    "port": port,
                    "database": database,
                    "type": "dev",
                    "auth-model": "native",
                    "provider-properties": { "sslmode": "disable" }
                }
            }
        }
    });
    std::fs::write(dir.join("data-sources.json"), sources.to_string()).expect("data sources");
    let credentials = json!({
        "postgres-jdbc-canary": { "#connection": { "user": user, "password": password } }
    });
    std::fs::write(
        dir.join("credentials-config.json"),
        encrypt_for_tests(&credentials.to_string(), [7u8; 16]),
    )
    .expect("credentials");
}

pub fn text_of(result: &CallToolResult) -> String {
    serde_json::to_value(result).expect("serializable result")["content"][0]["text"]
        .as_str()
        .expect("text content")
        .to_string()
}

pub fn body(result: &CallToolResult) -> Value {
    serde_json::from_str(&text_of(result)).expect("json body")
}

pub fn is_error(result: &CallToolResult) -> bool {
    result.is_error == Some(true)
}

pub fn assert_no_canary(response: &str) {
    for value in CANARY_VALUES {
        let forms = [
            value.to_string(),
            value.to_lowercase(),
            value.to_uppercase(),
            hex::encode(Md5::digest(value.as_bytes())),
            base64::engine::general_purpose::STANDARD.encode(value.as_bytes()),
        ];
        for form in forms {
            assert!(
                !response.contains(&form),
                "canary value {value:?} leaked as {form:?} in {response}"
            );
        }
        if let Some((local, _)) = value.split_once('@') {
            let characters: Vec<char> = local.chars().collect();
            for window in characters.windows(6) {
                let fragment: String = window.iter().collect();
                assert!(
                    !response.to_lowercase().contains(&fragment.to_lowercase()),
                    "fragment {fragment:?} of {value:?} leaked in {response}"
                );
            }
        }
    }
}
