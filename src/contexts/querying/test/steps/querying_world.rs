use std::sync::Arc;

use cucumber::World;
use pg_pseudo_mcp::common::AppConfig;
use pg_pseudo_mcp::composition::contexts::Contexts;
use pg_pseudo_mcp::contexts::anonymization::injector::AnonymizationInjector;
use pg_pseudo_mcp::contexts::querying::presentation::{QueryArgs, query};
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
        }
    }

    pub fn connect_to_test_database(&mut self) {
        write_workspace(self._dbeaver.path(), &database_target(), true, None);
        let config = AppConfig {
            dbeaver_home: Some(self._dbeaver.path().to_path_buf()),
            data_dir: Some(self._data.path().to_path_buf()),
            ssh_known_hosts: None,
        };
        let anonymization = AnonymizationInjector::new(self._data.path()).use_cases();
        self.contexts = Some(Arc::new(Contexts::with_anonymization(
            &config,
            anonymization,
        )));
    }

    pub async fn ask(&mut self, sql: &str) {
        let contexts = self
            .contexts
            .as_ref()
            .expect("connected to the test database");
        let result = query(
            &contexts.querying.run_query,
            QueryArgs {
                connection: CONNECTION.to_string(),
                sql: sql.to_string(),
                ..QueryArgs::default()
            },
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
