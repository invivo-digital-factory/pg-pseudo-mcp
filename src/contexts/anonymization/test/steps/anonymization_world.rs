use std::path::PathBuf;

use cucumber::World;
use scarif_database_mcp::common::AppConfig;
use scarif_database_mcp::contexts::anonymization::domain::services::pseudonymizer::Pseudonymizer;
use scarif_database_mcp::contexts::anonymization::domain::types::{
    AnonymizedDocuments, AnonymizedRows, Clearance, DatabaseMessage, FieldClearance, FieldPath,
    GeneratorConfig, Kind, Lineage, Node,
};
use scarif_database_mcp::contexts::anonymization::infrastructure::data_sources::data_directory::{
    DataDirectory, open_pseudonymizer,
};
use scarif_database_mcp::contexts::anonymization::injector::{
    AnonymizationInjector, AnonymizationUseCases, mapping_purge_handler,
};
use serde_json::Value;

#[derive(World)]
#[world(init = Self::new)]
pub struct AnonymizationWorld {
    pub data_dir: tempfile::TempDir,
    pub pseudonyms: Vec<String>,
    pub translation: Option<Option<String>>,
    pub document: Option<Node>,
    pub clearances: Vec<(FieldPath, FieldClearance)>,
    pub anonymized: Option<AnonymizedDocuments>,
    pub row_pseudonym: Option<String>,
    pub generator: GeneratorConfig,
    pub input: Option<String>,
    pub injected: Vec<(String, String)>,
    pub enum_labels: Vec<String>,
    pub column_clearances: Vec<Clearance>,
    pub lineage: Option<Vec<Lineage>>,
    pub clear_value: Option<Value>,
    pub rows: Option<AnonymizedRows>,
    pub recognised: Option<Option<Kind>>,
    pub dictionary: Option<Option<Kind>>,
    pub other_data_dir: Option<tempfile::TempDir>,
    pub data_home: Option<tempfile::TempDir>,
    pub explicit_dir: Option<PathBuf>,
    pub resolved: Option<Result<DataDirectory, String>>,
    pub legacy_pseudonym: Option<String>,
    pub open_error: Option<String>,
    pub known_names: Vec<String>,
    pub message: Option<DatabaseMessage>,
    pub plan: Option<Value>,
    pub canaries: Vec<String>,
}

impl std::fmt::Debug for AnonymizationWorld {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AnonymizationWorld")
            .field("pseudonyms", &self.pseudonyms)
            .field("translation", &self.translation)
            .field("document", &self.document)
            .field("row_pseudonym", &self.row_pseudonym)
            .field("input", &self.input)
            .field("message", &self.message)
            .field("plan", &self.plan)
            .finish_non_exhaustive()
    }
}

impl AnonymizationWorld {
    fn new() -> Self {
        Self {
            data_dir: tempfile::tempdir().expect("data dir"),
            pseudonyms: Vec::new(),
            translation: None,
            document: None,
            clearances: Vec::new(),
            anonymized: None,
            row_pseudonym: None,
            generator: GeneratorConfig::default(),
            input: None,
            injected: Vec::new(),
            enum_labels: Vec::new(),
            column_clearances: Vec::new(),
            lineage: None,
            clear_value: None,
            rows: None,
            recognised: None,
            dictionary: None,
            other_data_dir: None,
            data_home: None,
            explicit_dir: None,
            resolved: None,
            legacy_pseudonym: None,
            open_error: None,
            known_names: Vec::new(),
            message: None,
            plan: None,
            canaries: Vec::new(),
        }
    }

    pub fn use_cases(&self) -> AnonymizationUseCases {
        AnonymizationInjector::with_config(self.data_dir.path(), self.generator)
            .use_cases()
            .expect("pseudonym store opens")
    }

    pub fn use_cases_or_error(&self) -> Result<AnonymizationUseCases, String> {
        AnonymizationInjector::with_config(self.data_dir.path(), self.generator).use_cases()
    }

    pub fn pseudonymizer(&self) -> Pseudonymizer {
        open_pseudonymizer(self.data_dir.path(), self.generator).expect("pseudonym store opens")
    }

    pub fn last_pseudonym(&self) -> &str {
        self.pseudonyms.last().expect("a pseudonym was issued")
    }

    pub fn nth_pseudonym(&self, position: usize) -> &str {
        self.pseudonyms
            .get(position - 1)
            .unwrap_or_else(|| panic!("no pseudonym number {position} in {:?}", self.pseudonyms))
    }

    pub fn input(&self) -> &str {
        self.input.as_deref().expect("an input value")
    }

    pub fn expand(&self, text: &str) -> String {
        match self.pseudonyms.first() {
            Some(pseudonym) => text.replace("{pseudonyme}", pseudonym),
            None => text.to_string(),
        }
    }

    pub fn purge(&self) {
        let ssh_config = self.data_dir.path().join("ssh_config");
        std::fs::write(&ssh_config, "").expect("empty ssh config");
        let config = AppConfig {
            dbeaver_home: None,
            data_dir: Some(self.data_dir.path().to_path_buf()),
            ssh_known_hosts: None,
            ssh_config: Some(ssh_config.clone()),
            connections_file: Some(self.data_dir.path().join("absent-connections.toml")),
            columns_file: None,
            xdg_config_home: None,
            home_dir: None,
            data_home: None,
        };
        let handler = mapping_purge_handler(&config).expect("purge handler");
        let status = handler.run(true);
        std::fs::remove_file(ssh_config).expect("ssh config removed");
        assert_eq!(
            format!("{status:?}"),
            format!("{:?}", std::process::ExitCode::SUCCESS)
        );
    }
}

pub fn listed(values: &str) -> Vec<String> {
    values
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

pub fn kind_named(name: &str) -> Kind {
    Kind::parse(name).unwrap_or_else(|| panic!("unknown kind {name}"))
}

pub fn layout(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_ascii_alphanumeric() { 'x' } else { c })
        .collect()
}

pub fn digits(text: &str) -> String {
    text.chars().filter(char::is_ascii_digit).collect()
}

pub fn quoted(text: &str) -> String {
    text.replace("\\\"", "\"")
}
