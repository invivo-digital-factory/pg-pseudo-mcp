use std::sync::Arc;

use cucumber::World;
use scarif_database_mcp::common::AppConfig;
use scarif_database_mcp::composition::contexts::Contexts;
use scarif_database_mcp::contexts::anonymization::injector::AnonymizationInjector;
use scarif_database_mcp::contexts::connections::presentation::list_connections_with_warnings;
use scarif_database_mcp::contexts::querying::presentation::{
    DescribeTableArgs, ListDatabasesArgs, QueryArgs, describe_table, list_databases, query,
};
use serde_json::Value;

use crate::bdd_support::{CONNECTION, database_target, is_error, text_of, write_workspace};

pub struct Answer {
    pub refused: bool,
    pub text: String,
}

impl Answer {
    pub fn body(&self) -> Value {
        serde_json::from_str(&self.text).expect("json body")
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
        }
    }

    pub fn connect_to_test_database(&mut self) {
        write_workspace(self._dbeaver.path(), &database_target(), true, None);
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
        let config = AppConfig {
            dbeaver_home: Some(self._dbeaver.path().to_path_buf()),
            data_dir: Some(self._data.path().to_path_buf()),
            ssh_known_hosts: None,
            ssh_config: Some(ssh_config),
            connections_file: Some(self._data.path().join("absent-connections.toml")),
            columns_file,
            xdg_config_home,
            home_dir: None,
        };
        let anonymization = AnonymizationInjector::new(self._data.path()).use_cases();
        self.contexts = Some(Arc::new(Contexts::with_anonymization(
            &config,
            anonymization,
        )));
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
        self.answer = Some(Answer {
            refused: is_error(&result),
            text: text_of(&result),
        });
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
        self.answer = Some(Answer {
            refused: is_error(&result),
            text: text_of(&result),
        });
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
        self.answer = Some(Answer {
            refused: is_error(&result),
            text: text_of(&result),
        });
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
        self.answer = Some(Answer {
            refused: is_error(&result),
            text: text_of(&result),
        });
    }

    pub fn answer(&self) -> &Answer {
        self.answer.as_ref().expect("a query was asked")
    }
}
