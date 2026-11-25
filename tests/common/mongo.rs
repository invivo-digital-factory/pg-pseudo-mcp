use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::model::CallToolResult;
use scarif_database_mcp::common::AppConfig;
use scarif_database_mcp::composition::contexts::Contexts;
use scarif_database_mcp::composition::tools;
use scarif_database_mcp::contexts::anonymization::injector::AnonymizationInjector;
use scarif_database_mcp::contexts::querying::presentation::{
    AggregateArgs, DescribeTableArgs, ExplainArgs, FindArgs, ListDatabasesArgs, ListTablesArgs,
    QueryArgs, SearchSchemaArgs,
};
use serde_json::Value;

pub const GATE_VAR: &str = "SCARIF_TEST_MONGODB";
pub const SSH_PORT_VAR: &str = "SCARIF_TEST_SSH_PORT";
pub const MONGO: &str = "atlas-local";
pub const DATABASE: &str = "shop";
pub const READER: (&str, &str) = ("atlas-reader", "atlas-reader-test-pw");
pub const WRITER: (&str, &str) = ("atlas-writer", "atlas-writer-test-pw");
pub const WILDCARD_POLICY: &str = r#"
[mongodb.clear]
"shop.attributes" = ["Select_Options.*.Label", "*.red.owner", "reference_entity_options.*"]

[mongodb.personal."shop.attributes"]
"select_options.*.owner" = "person_name"
"select_options.vip.label" = "last_name"
"#;
const BASTION_ALIAS: &str = "bastion-test";
const BASTION_HOST: &str = "localhost";
const BASTION_USER: &str = "tunnel";

pub fn require_gate() {
    assert!(
        std::env::var(GATE_VAR).is_ok_and(|value| !value.is_empty()),
        "{GATE_VAR} must be set to run the tests that need the docker compose mongodb service"
    );
}

fn bastion_port() -> u16 {
    std::env::var(SSH_PORT_VAR)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(55222)
}

fn ssh_asset(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("ssh")
        .join(name)
}

fn known_hosts_line() -> String {
    let line = std::fs::read_to_string(ssh_asset("ssh_host_ed25519_test.pub")).expect("host key");
    let mut fields = line.split_whitespace();
    let algorithm = fields.next().expect("key algorithm");
    let key = fields.next().expect("key data");
    let pattern = match bastion_port() {
        22 => BASTION_HOST.to_string(),
        port => format!("[{BASTION_HOST}]:{port}"),
    };
    format!("{pattern} {algorithm} {key}\n")
}

pub struct MongoHarness {
    pub state: Arc<Contexts>,
    _dir: tempfile::TempDir,
}

impl MongoHarness {
    pub fn new() -> Self {
        Self::with(READER, None, Some(DATABASE))
    }

    pub fn as_user(account: (&str, &str)) -> Self {
        Self::with(account, None, Some(DATABASE))
    }

    pub fn with_policy(policy: &str) -> Self {
        Self::with(READER, Some(policy), Some(DATABASE))
    }

    pub fn without_default_database() -> Self {
        Self::with(READER, None, None)
    }

    fn with(account: (&str, &str), policy: Option<&str>, database: Option<&str>) -> Self {
        require_gate();
        let dir = tempfile::tempdir().expect("harness dir");
        let root = dir.path();
        let ssh_config = root.join("ssh_config");
        std::fs::write(
            &ssh_config,
            format!(
                "Host {BASTION_ALIAS}\n  HostName {BASTION_HOST}\n  Port {}\n  User {BASTION_USER}\n  IdentityFile {}\n  IdentitiesOnly yes\n  IdentityAgent none\n",
                bastion_port(),
                ssh_asset("tunnel_test_ed25519").display()
            ),
        )
        .expect("ssh config");
        let known_hosts = root.join("known_hosts");
        std::fs::write(&known_hosts, known_hosts_line()).expect("known hosts");
        let database_line = database
            .map(|name| format!("database = \"{name}\"\n"))
            .unwrap_or_default();
        let connections = root.join("connections.toml");
        std::fs::write(
            &connections,
            format!(
                "[[connection]]\nname = \"{MONGO}\"\nengine = \"mongodb\"\nhost = \"pl-0.atlas.scarif.test\"\nsrv = false\nport = 1024\ntls = false\nreplica_set = \"rs0\"\n{database_line}user = \"{}\"\ntype = \"test\"\nauth = \"command\"\npassword_command = [\"/bin/echo\", \"{}\"]\nssh_alias = \"{BASTION_ALIAS}\"\n",
                account.0, account.1
            ),
        )
        .expect("connection file");
        let columns_file = policy.map(|contents| {
            let path = root.join("columns.toml");
            std::fs::write(&path, contents).expect("column policy");
            path
        });
        let dbeaver = root.join("dbeaver");
        std::fs::create_dir_all(&dbeaver).expect("dbeaver dir");
        let config = AppConfig {
            dbeaver_home: Some(dbeaver),
            data_dir: Some(root.join("data")),
            ssh_known_hosts: Some(known_hosts),
            ssh_config: Some(ssh_config),
            connections_file: Some(connections),
            columns_file,
            xdg_config_home: None,
            home_dir: None,
            data_home: None,
        };
        let anonymization = AnonymizationInjector::new(&root.join("data")).use_cases();
        Self {
            state: Arc::new(Contexts::with_anonymization(&config, anonymization)),
            _dir: dir,
        }
    }

    pub async fn find(&self, collection: &str, args: FindArgs) -> CallToolResult {
        tools::find(
            &self.state,
            FindArgs {
                connection: MONGO.into(),
                collection: collection.into(),
                ..args
            },
        )
        .await
    }

    pub async fn aggregate(&self, collection: &str, pipeline: Value) -> CallToolResult {
        self.aggregate_with(
            collection,
            AggregateArgs {
                pipeline: pipeline.as_array().expect("pipeline array").clone(),
                ..AggregateArgs::default()
            },
        )
        .await
    }

    pub async fn aggregate_with(&self, collection: &str, args: AggregateArgs) -> CallToolResult {
        tools::aggregate(
            &self.state,
            AggregateArgs {
                connection: MONGO.into(),
                collection: collection.into(),
                ..args
            },
        )
        .await
    }

    pub async fn list_databases(&self) -> CallToolResult {
        tools::list_databases(
            &self.state,
            ListDatabasesArgs {
                connection: MONGO.into(),
            },
        )
        .await
    }

    pub async fn list_tables(&self) -> CallToolResult {
        tools::list_tables(
            &self.state,
            ListTablesArgs {
                connection: MONGO.into(),
                ..ListTablesArgs::default()
            },
        )
        .await
    }

    pub async fn describe(&self, collection: &str) -> CallToolResult {
        tools::describe_table(
            &self.state,
            DescribeTableArgs {
                connection: MONGO.into(),
                table: collection.into(),
                database: None,
            },
        )
        .await
    }

    pub async fn search(&self, pattern: &str) -> CallToolResult {
        tools::search_schema(
            &self.state,
            SearchSchemaArgs {
                connection: MONGO.into(),
                pattern: pattern.into(),
                database: None,
            },
        )
        .await
    }

    pub async fn query(&self, sql: &str) -> CallToolResult {
        tools::query(
            &self.state,
            QueryArgs {
                connection: MONGO.into(),
                sql: sql.into(),
                ..QueryArgs::default()
            },
        )
        .await
    }

    pub async fn explain(&self, sql: &str) -> CallToolResult {
        tools::explain(
            &self.state,
            ExplainArgs {
                connection: MONGO.into(),
                sql: sql.into(),
                database: None,
            },
        )
        .await
    }
}

pub fn filter(value: Value) -> FindArgs {
    FindArgs {
        filter: Some(value),
        ..FindArgs::default()
    }
}
