use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use cucumber::World;
use scarif_database_mcp::common::AppConfig;
use scarif_database_mcp::composition::contexts::Contexts;
use scarif_database_mcp::contexts::anonymization::injector::AnonymizationInjector;
use scarif_database_mcp::contexts::connections::presentation::list_connections_with_warnings;
use scarif_database_mcp::contexts::querying::domain::repositories::PseudonymTranslator;
use scarif_database_mcp::contexts::querying::domain::services::mongo_lineage::DocumentLineage;
use scarif_database_mcp::contexts::querying::domain::types::{
    ClampWarning, ColumnPolicy, FieldPolicyHint, Guarded, GuardedMongo, MongoCommand,
    PseudonymLookup, Warning,
};
use scarif_database_mcp::contexts::querying::presentation::{
    DescribeTableArgs, ListDatabasesArgs, QueryArgs, describe_table, list_databases, query,
};
use serde_json::Value;

use crate::bdd_support::{
    BASTION_HOST, BASTION_USER, CONNECTION, bastion_port, database_target, is_error,
    known_hosts_pattern, offline_target, public_key_fields, ssh_asset, text_of, write_workspace,
};

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/common/canary_values.rs"
));

pub const MONGO_CONNECTION: &str = "atlas-test";

pub fn refusal_text(code: String, message: String, hint: Option<String>) -> String {
    format!("{code} {message} {}", hint.unwrap_or_default())
}

pub struct MongoUser {
    pub name: &'static str,
    pub password: &'static str,
    pub default_database: bool,
}

pub const MONGO_READER: MongoUser = MongoUser {
    name: "atlas-reader",
    password: "atlas-reader-test-pw",
    default_database: true,
};

#[derive(Default)]
pub struct PseudonymBook {
    pseudonyms: HashMap<String, String>,
    lookups: AtomicUsize,
}

impl PseudonymBook {
    pub fn new(pseudonyms: &[(String, String)]) -> Self {
        Self {
            pseudonyms: pseudonyms.iter().cloned().collect(),
            lookups: AtomicUsize::new(0),
        }
    }

    pub fn lookups(&self) -> usize {
        self.lookups.load(Ordering::SeqCst)
    }
}

impl PseudonymTranslator for PseudonymBook {
    fn lookup(&self, text: &str) -> PseudonymLookup {
        self.lookups.fetch_add(1, Ordering::SeqCst);
        if let Some(real) = self.pseudonyms.get(text) {
            return PseudonymLookup::Known { real: real.clone() };
        }
        let domain = text.rsplit_once('@').map(|(_, domain)| domain);
        if matches!(domain, Some("example.com" | "example.net" | "example.org")) {
            PseudonymLookup::Unknown
        } else {
            PseudonymLookup::NotAPseudonym
        }
    }
}

pub struct Answer {
    pub refused: bool,
    pub text: String,
}

impl Answer {
    pub fn body(&self) -> Value {
        serde_json::from_str(&self.text).expect("json body")
    }

    pub fn checked_body(&self) -> Value {
        assert!(!self.refused, "{}", self.text);
        self.body()
    }

    pub fn rows(&self) -> Vec<Vec<Value>> {
        self.body()["rows"]
            .as_array()
            .expect("rows array")
            .iter()
            .map(|row| row.as_array().expect("row array").clone())
            .collect()
    }
}

pub struct MongoAnswer {
    pub outcome: Result<GuardedMongo, String>,
    pub lookups: usize,
    pub clamps: Vec<ClampWarning>,
}

#[derive(World)]
#[world(init = Self::new)]
pub struct QueryingWorld {
    contexts: Option<Arc<Contexts>>,
    _dbeaver: tempfile::TempDir,
    _data: tempfile::TempDir,
    pub answer: Option<Answer>,
    pub pseudonym: Option<String>,
    pub shared_policy: Option<String>,
    pub local_policy: Option<String>,
    pub policy: ColumnPolicy,
    pub policy_warnings: Vec<String>,
    pub hint: Option<FieldPolicyHint>,
    pub pseudonyms: Vec<(String, String)>,
    pub mongo: Option<MongoAnswer>,
    pub sql: Option<Result<Guarded, String>>,
    pub pipeline: Option<Vec<Value>>,
    pub lineage: Option<DocumentLineage>,
}

impl std::fmt::Debug for QueryingWorld {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("QueryingWorld")
            .field("pseudonym", &self.pseudonym)
            .finish_non_exhaustive()
    }
}

impl QueryingWorld {
    fn new() -> Self {
        Self {
            contexts: None,
            _dbeaver: tempfile::tempdir().expect("dbeaver dir"),
            _data: tempfile::tempdir().expect("data dir"),
            answer: None,
            pseudonym: None,
            shared_policy: None,
            local_policy: None,
            policy: ColumnPolicy::default(),
            policy_warnings: Vec::new(),
            hint: None,
            pseudonyms: Vec::new(),
            mongo: None,
            sql: None,
            pipeline: None,
            lineage: None,
        }
    }

    fn config(&self) -> AppConfig {
        let ssh_config = self._data.path().join("ssh_config");
        std::fs::write(&ssh_config, "").expect("empty ssh config");
        let columns_file = self.shared_policy.as_ref().map(|contents| {
            let path = self._data.path().join("team-columns.toml");
            std::fs::write(&path, contents).expect("shared column policy");
            path
        });
        let xdg_config_home = self.local_policy.as_ref().map(|contents| {
            let folder = self._data.path().join("xdg").join("scarif");
            std::fs::create_dir_all(&folder).expect("local config folder");
            std::fs::write(folder.join("columns.toml"), contents).expect("local column policy");
            self._data.path().join("xdg").into_os_string()
        });
        AppConfig {
            dbeaver_home: Some(self._dbeaver.path().to_path_buf()),
            data_dir: Some(self._data.path().to_path_buf()),
            ssh_known_hosts: None,
            ssh_config: Some(ssh_config),
            connections_file: Some(self._data.path().join("absent-connections.toml")),
            columns_file,
            xdg_config_home,
            home_dir: None,
            data_home: None,
        }
    }

    fn open_contexts(&mut self, config: &AppConfig) {
        let anonymization = AnonymizationInjector::new(self._data.path()).use_cases();
        self.contexts = Some(Arc::new(Contexts::with_anonymization(
            config,
            anonymization,
        )));
    }

    pub fn connect_to_test_database(&mut self) {
        write_workspace(self._dbeaver.path(), &database_target(), true, None);
        let config = self.config();
        self.open_contexts(&config);
    }

    pub fn connect_to_test_database_on_prod(&mut self, extra: &str) {
        let target = database_target();
        let connections = self._data.path().join("connections.toml");
        std::fs::write(
            &connections,
            format!(
                "[[connection]]\nname = \"{CONNECTION}\"\nhost = \"{}\"\nport = {}\ndatabase = \"{}\"\nuser = \"{}\"\nsslmode = \"disable\"\ntype = \"prod\"\nauth = \"command\"\npassword_command = [\"/bin/echo\", \"{}\"]\n{extra}\n",
                target.host, target.port, target.database, target.user, target.password
            ),
        )
        .expect("connection file");
        let config = AppConfig {
            connections_file: Some(connections),
            ..self.config()
        };
        self.open_contexts(&config);
    }

    pub fn connect_to_unreachable_database(&mut self) {
        write_workspace(self._dbeaver.path(), &offline_target(), true, None);
        let config = self.config();
        self.open_contexts(&config);
    }

    pub fn connect_without_pseudonym_store(&mut self) {
        write_workspace(self._dbeaver.path(), &offline_target(), true, None);
        let config = self.config();
        self.contexts = Some(Arc::new(Contexts::with_anonymization(
            &config,
            Err("the pseudonym secret cannot be read".to_string()),
        )));
    }

    pub fn connect_to_test_mongodb(&mut self) {
        self.connect_to_test_mongodb_as(&MONGO_READER);
    }

    pub fn connect_to_test_mongodb_as(&mut self, user: &MongoUser) {
        self.connect_to_test_mongodb_with(user, "type = \"test\"");
    }

    pub fn connect_to_test_mongodb_on_prod(&mut self, extra: &str) {
        self.connect_to_test_mongodb_with(&MONGO_READER, &format!("type = \"prod\"\n{extra}"));
    }

    fn connect_to_test_mongodb_with(&mut self, user: &MongoUser, kind: &str) {
        let root = self._data.path();
        let ssh_config = root.join("ssh_config_mongodb");
        std::fs::write(
            &ssh_config,
            format!(
                "Host bastion-test\n  HostName {BASTION_HOST}\n  Port {}\n  User {BASTION_USER}\n  IdentityFile {}\n  IdentitiesOnly yes\n  IdentityAgent none\n",
                bastion_port(),
                ssh_asset("tunnel_test_ed25519").display()
            ),
        )
        .expect("ssh config");
        let known_hosts = root.join("known_hosts");
        std::fs::write(
            &known_hosts,
            format!(
                "{} {}\n",
                known_hosts_pattern(),
                public_key_fields("ssh_host_ed25519_test.pub")
            ),
        )
        .expect("known hosts");
        let connections = root.join("connections.toml");
        let database = if user.default_database {
            "database = \"shop\"\n"
        } else {
            ""
        };
        std::fs::write(
            &connections,
            format!(
                "[[connection]]\nname = \"{MONGO_CONNECTION}\"\nengine = \"mongodb\"\nhost = \"pl-0.atlas.scarif.test\"\nsrv = false\nport = 1024\ntls = false\nreplica_set = \"rs0\"\n{database}user = \"{}\"\n{kind}\nauth = \"command\"\npassword_command = [\"/bin/echo\", \"{}\"]\nssh_alias = \"bastion-test\"\n",
                user.name, user.password
            ),
        )
        .expect("connection file");
        let config = AppConfig {
            ssh_known_hosts: Some(known_hosts),
            ssh_config: Some(ssh_config),
            connections_file: Some(connections),
            ..self.config()
        };
        self.open_contexts(&config);
    }

    pub fn contexts(&self) -> &Contexts {
        self.contexts.as_ref().expect("contexts opened")
    }

    pub fn record(&mut self, result: &rmcp::model::CallToolResult) {
        self.sql = None;
        self.mongo = None;
        self.answer = Some(Answer {
            refused: is_error(result),
            text: text_of(result),
        });
    }

    pub fn record_sql_guard(&mut self, outcome: Result<Guarded, String>) {
        self.answer = None;
        self.mongo = None;
        self.sql = Some(outcome);
    }

    pub fn record_mongo_guard(&mut self, outcome: MongoAnswer) {
        self.answer = None;
        self.sql = None;
        self.mongo = Some(outcome);
    }

    pub fn load_column_policy(&mut self) {
        let config = self.config();
        self.open_contexts(&config);
        let load = self
            .contexts
            .as_ref()
            .expect("contexts")
            .querying
            .read_column_policy
            .execute();
        self.policy = load.policy;
        self.policy_warnings = load.warnings;
    }

    pub async fn ask(&mut self, sql: &str) {
        self.ask_in(sql, None).await;
    }

    pub async fn list_databases(&mut self) {
        let contexts = self
            .contexts
            .as_ref()
            .expect("connected to the test database");
        let result = list_databases(
            &contexts.querying.list_databases,
            ListDatabasesArgs {
                connection: CONNECTION.to_string(),
            },
        )
        .await;
        self.record(&result);
    }

    pub async fn ask_in(&mut self, sql: &str, database: Option<&str>) {
        let contexts = self
            .contexts
            .as_ref()
            .expect("connected to the test database");
        let result = query(
            &contexts.querying.run_query,
            QueryArgs {
                connection: CONNECTION.to_string(),
                sql: sql.to_string(),
                database: database.map(str::to_string),
                ..QueryArgs::default()
            },
        )
        .await;
        self.record(&result);
    }

    pub async fn describe(&mut self, table: &str) {
        let contexts = self
            .contexts
            .as_ref()
            .expect("connected to the test database");
        let result = describe_table(
            &contexts.querying.describe_table,
            DescribeTableArgs {
                connection: CONNECTION.to_string(),
                table: table.to_string(),
                database: None,
            },
        )
        .await;
        self.record(&result);
    }

    pub async fn list_connections(&mut self) {
        let contexts = self
            .contexts
            .as_ref()
            .expect("connected to the test database");
        let result = list_connections_with_warnings(
            &contexts.connections.list_connections,
            &contexts.column_policy_warnings(),
        )
        .await;
        self.record(&result);
    }

    pub async fn ask_with(&mut self, args: QueryArgs) {
        let contexts = self.contexts.as_ref().expect("contexts opened");
        let result = query(
            &contexts.querying.run_query,
            QueryArgs {
                connection: CONNECTION.to_string(),
                ..args
            },
        )
        .await;
        self.record(&result);
    }

    pub fn mongo_answer(&self) -> &MongoAnswer {
        self.mongo.as_ref().expect("a MongoDB request was guarded")
    }

    pub fn refusal_text(&self) -> String {
        if let Some(mongo) = &self.mongo {
            return mongo.outcome.clone().expect_err("a refusal");
        }
        if let Some(sql) = &self.sql {
            return sql.clone().expect_err("a refusal");
        }
        let answer = self.answer();
        assert!(answer.refused, "{}", answer.text);
        let error = &answer.body()["error"];
        format!(
            "{} {} {}",
            error["code"].as_str().unwrap_or_default(),
            error["message"].as_str().unwrap_or_default(),
            error["hint"].as_str().unwrap_or_default()
        )
    }

    pub fn accepted_translations(&self) -> (usize, Vec<Warning>, String) {
        if let Some(sql) = &self.sql {
            let guarded = sql
                .as_ref()
                .unwrap_or_else(|refusal| panic!("refused: {refusal}"));
            return (
                guarded.translated_literals,
                guarded.warnings.clone(),
                guarded.sql.clone(),
            );
        }
        let guarded = self
            .mongo_answer()
            .outcome
            .clone()
            .unwrap_or_else(|refusal| panic!("refused: {refusal}"));
        let sent = match guarded.command {
            MongoCommand::Find(command) => command.filter.to_string(),
            MongoCommand::Aggregate(command) => {
                serde_json::Value::Array(command.pipeline).to_string()
            }
        };
        (guarded.translated_literals, guarded.warnings, sent)
    }

    pub fn answer(&self) -> &Answer {
        self.answer.as_ref().expect("a query was asked")
    }
}
