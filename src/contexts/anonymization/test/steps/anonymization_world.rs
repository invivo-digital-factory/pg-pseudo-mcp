use cucumber::World;
use scarif_database_mcp::common::AppConfig;
use scarif_database_mcp::contexts::anonymization::domain::types::{
    AnonymizedDocuments, FieldClearance, FieldPath, Node,
};
use scarif_database_mcp::contexts::anonymization::injector::{
    AnonymizationInjector, AnonymizationUseCases, mapping_purge_handler,
};

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
}

impl std::fmt::Debug for AnonymizationWorld {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AnonymizationWorld")
            .field("pseudonyms", &self.pseudonyms)
            .field("translation", &self.translation)
            .field("document", &self.document)
            .field("row_pseudonym", &self.row_pseudonym)
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
        }
    }

    pub fn use_cases(&self) -> AnonymizationUseCases {
        AnonymizationInjector::new(self.data_dir.path())
            .use_cases()
            .expect("pseudonym store opens")
    }

    pub fn purge(&self) {
        let ssh_config = self.data_dir.path().join("ssh_config");
        std::fs::write(&ssh_config, "").expect("empty ssh config");
        let config = AppConfig {
            dbeaver_home: None,
            data_dir: Some(self.data_dir.path().to_path_buf()),
            ssh_known_hosts: None,
            ssh_config: Some(ssh_config),
            connections_file: Some(self.data_dir.path().join("absent-connections.toml")),
            columns_file: None,
            xdg_config_home: None,
            home_dir: None,
            data_home: None,
        };
        let handler = mapping_purge_handler(&config).expect("purge handler");
        let status = handler.run(true);
        assert_eq!(
            format!("{status:?}"),
            format!("{:?}", std::process::ExitCode::SUCCESS)
        );
    }
}
