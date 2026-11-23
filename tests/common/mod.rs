#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;

use base64::Engine;
use md5::{Digest, Md5};
use pg_pseudo_mcp::common::AppConfig;
use pg_pseudo_mcp::composition::contexts::Contexts;
use pg_pseudo_mcp::contexts::anonymization::injector::AnonymizationInjector;
use pg_pseudo_mcp::contexts::connections::infrastructure::data_sources::dbeaver::credentials::encrypt_for_tests;
use pg_pseudo_mcp::contexts::connections::presentation;
use pg_pseudo_mcp::contexts::querying::presentation as querying;
use rmcp::model::CallToolResult;
use serde_json::{Value, json};

pub use pg_pseudo_mcp::contexts::querying::presentation::{
    DescribeTableArgs, ExplainArgs, ListDatabasesArgs, ListTablesArgs, QueryArgs, SearchSchemaArgs,
};

pub const DATABASE_URL_VAR: &str = "PG_PSEUDO_TEST_DATABASE_URL";
pub const CONNECTION: &str = "canary-local";

pub const CANARY_VALUES: &[&str] = &[
    "Ylwenna",
    "Kervadec",
    "ylwenna.kervadec@canary-mail.fr",
    "06 41 27 83 95",
    "33641278395",
    "FR76 3000 4000 0312 3456 7890 143",
    "418273652",
    "2851175123456 22",
    "o.hazvekmor@canary-mail.fr",
    "kervadec.ylwenna@canary-mail.fr",
    "ylwenna.k@canary-mail.fr",
    "yk.pro@canary-mail.fr",
    "Odrigan",
    "Hazvekmor",
    "odrigan.hazvekmor@canary-mail.fr",
    "07 58 14 92 36",
    "33758149236",
    "FR14 2004 1010 0505 0001 3M02 606",
    "odrigan.h@canary-mail.fr",
    "Maëlouz",
    "Brizolier",
    "maelouz.brizolier@canary-mail.fr",
    "Ylwenna Kervadec",
    "Odrigan Hazvekmor",
    "06 52 18 74 30",
    "12 rue des Lilas",
    "zivrel.quanteb@canary-mail.fr",
];

pub fn database_url() -> String {
    std::env::var(DATABASE_URL_VAR)
        .unwrap_or_else(|_| panic!("{DATABASE_URL_VAR} must point at the docker compose database"))
}

pub struct Harness {
    pub state: Arc<Contexts>,
    _dbeaver: tempfile::TempDir,
    _data: tempfile::TempDir,
}

pub fn isolated_ssh_config(dir: &Path) -> std::path::PathBuf {
    let path = dir.join("ssh_config");
    std::fs::write(&path, "").expect("empty ssh config");
    path
}

impl Harness {
    pub fn new() -> Self {
        Self::with_workspace(write_workspace_for_test_database)
    }

    pub fn with_login(user: &str, password: &str) -> Self {
        Self::with_workspace(|root| {
            let url = database_url();
            let (_, location) = url
                .strip_prefix("postgres://")
                .and_then(|rest| rest.split_once('@'))
                .expect("user:password@host");
            let (address, database) = location.split_once('/').expect("host:port/database");
            let (host, port) = address.split_once(':').expect("host:port");
            write_workspace(root, host, port, database, user, password);
        })
    }

    pub fn with_password_command(program: &str, arguments: &[&str]) -> Self {
        let url = database_url();
        let (credentials, location) = url
            .strip_prefix("postgres://")
            .and_then(|rest| rest.split_once('@'))
            .expect("user:password@host");
        let (user, _) = credentials.split_once(':').expect("user:password");
        let (address, database) = location.split_once('/').expect("host:port/database");
        let (host, port) = address.split_once(':').expect("host:port");
        let mut command = vec![program.to_string()];
        command.extend(arguments.iter().map(ToString::to_string));
        let file = format!(
            "[[connection]]\nname = \"{CONNECTION}\"\nhost = \"{host}\"\nport = {port}\ndatabase = \"{database}\"\nuser = \"{user}\"\nsslmode = \"disable\"\ntype = \"dev\"\nauth = \"command\"\npassword_command = {}\n",
            serde_json::to_string(&command).expect("json array is a toml array")
        );
        let dbeaver = tempfile::tempdir().expect("dbeaver dir");
        let data = tempfile::tempdir().expect("data dir");
        let path = data.path().join("connections.toml");
        std::fs::write(&path, file).expect("connection file");
        let config = AppConfig {
            dbeaver_home: Some(dbeaver.path().to_path_buf()),
            data_dir: Some(data.path().to_path_buf()),
            ssh_known_hosts: None,
            ssh_config: Some(isolated_ssh_config(data.path())),
            connections_file: Some(path),
            columns_file: None,
            xdg_config_home: None,
            home_dir: None,
        };
        let pseudonymizer = AnonymizationInjector::new(data.path()).use_cases();
        Self {
            state: Arc::new(Contexts::with_anonymization(&config, pseudonymizer)),
            _dbeaver: dbeaver,
            _data: data,
        }
    }

    pub fn with_column_policy(shared: Option<&str>, local: Option<&str>) -> Self {
        Self::with_workspace_and_policy(write_workspace_for_test_database, shared, local)
    }

    fn with_workspace(write: impl FnOnce(&Path)) -> Self {
        Self::with_workspace_and_policy(write, None, None)
    }

    fn with_workspace_and_policy(
        write: impl FnOnce(&Path),
        shared: Option<&str>,
        local: Option<&str>,
    ) -> Self {
        let dbeaver = tempfile::tempdir().expect("dbeaver dir");
        write(dbeaver.path());
        let data = tempfile::tempdir().expect("data dir");
        let columns_file = shared.map(|contents| {
            let path = data.path().join("team-columns.toml");
            std::fs::write(&path, contents).expect("shared column policy");
            path
        });
        let xdg_config_home = local.map(|contents| {
            let folder = data.path().join("xdg").join("pg-pseudo");
            std::fs::create_dir_all(&folder).expect("local config folder");
            std::fs::write(folder.join("columns.toml"), contents).expect("local column policy");
            data.path().join("xdg").into_os_string()
        });
        let config = AppConfig {
            dbeaver_home: Some(dbeaver.path().to_path_buf()),
            data_dir: Some(data.path().to_path_buf()),
            ssh_known_hosts: None,
            ssh_config: Some(isolated_ssh_config(data.path())),
            connections_file: Some(data.path().join("absent-connections.toml")),
            columns_file,
            xdg_config_home,
            home_dir: None,
        };
        let pseudonymizer = AnonymizationInjector::new(data.path()).use_cases();
        Self {
            state: Arc::new(Contexts::with_anonymization(&config, pseudonymizer)),
            _dbeaver: dbeaver,
            _data: data,
        }
    }
}

pub async fn list_connections(contexts: &Contexts) -> CallToolResult {
    presentation::list_connections(&contexts.connections.list_connections).await
}

pub async fn list_connections_with_policy_warnings(contexts: &Contexts) -> CallToolResult {
    presentation::list_connections_with_warnings(
        &contexts.connections.list_connections,
        &contexts.column_policy_warnings(),
    )
    .await
}

pub async fn query(contexts: &Contexts, args: QueryArgs) -> CallToolResult {
    querying::query(&contexts.querying.run_query, args).await
}

pub async fn explain(contexts: &Contexts, args: ExplainArgs) -> CallToolResult {
    querying::explain(&contexts.querying.explain_query, args).await
}

pub async fn search_schema(contexts: &Contexts, args: SearchSchemaArgs) -> CallToolResult {
    querying::search_schema(&contexts.querying.search_schema, args).await
}

pub async fn list_databases(contexts: &Contexts, args: ListDatabasesArgs) -> CallToolResult {
    querying::list_databases(&contexts.querying.list_databases, args).await
}

pub async fn list_tables(contexts: &Contexts, args: ListTablesArgs) -> CallToolResult {
    querying::list_tables(&contexts.querying.list_tables, args).await
}

pub async fn describe_table(contexts: &Contexts, args: DescribeTableArgs) -> CallToolResult {
    querying::describe_table(&contexts.querying.describe_table, args).await
}

pub fn write_workspace_for_test_database(root: &Path) {
    let url = database_url();
    let rest = url.strip_prefix("postgres://").expect("postgres:// url");
    let (credentials, location) = rest.split_once('@').expect("user:password@host");
    let (user, password) = credentials.split_once(':').expect("user:password");
    let (address, database) = location.split_once('/').expect("host:port/database");
    let (host, port) = address.split_once(':').expect("host:port");
    write_workspace(root, host, port, database, user, password);
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
        let mut forms = vec![
            value.to_string(),
            value.to_lowercase(),
            value.to_uppercase(),
            hex::encode(Md5::digest(value.as_bytes())),
            base64::engine::general_purpose::STANDARD.encode(value.as_bytes()),
            hex::encode(value.as_bytes()),
            hex::encode_upper(value.as_bytes()),
        ];
        if let Ok(number) = value.parse::<u64>() {
            forms.push(format!("{number:x}"));
            forms.push(format!("{number:X}"));
        }
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
