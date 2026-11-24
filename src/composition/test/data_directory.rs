use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use tempfile::TempDir;

use crate::common::AppConfig;
use crate::composition::contexts::Contexts;
use crate::composition::notices::data_directory_notices;
use crate::composition::server_info::server_info;
use crate::contexts::anonymization::domain::types::{GeneratorConfig, Kind};
use crate::contexts::anonymization::infrastructure::data_sources::data_directory::{
    SECRET_FILE, STORE_FILE, open_pseudonymizer,
};
use crate::contexts::anonymization::injector::AnonymizationInjector;

const EMAIL: &str = "ylwenna.kervadec@canary-mail.fr";

struct Machine {
    root: TempDir,
}

impl Machine {
    fn new() -> Self {
        let machine = Self {
            root: TempDir::new().unwrap(),
        };
        fs::write(machine.root.path().join("ssh_config"), "").unwrap();
        machine
    }

    fn data_home(&self) -> PathBuf {
        self.root.path().join("data-home")
    }

    fn preferred(&self) -> PathBuf {
        self.data_home().join("scarif-database-mcp")
    }

    fn legacy(&self) -> PathBuf {
        self.data_home().join("pg-pseudo-mcp")
    }

    fn config(&self, data_dir: Option<PathBuf>) -> AppConfig {
        AppConfig {
            dbeaver_home: Some(self.root.path().join("dbeaver")),
            data_dir,
            ssh_config: Some(self.root.path().join("ssh_config")),
            connections_file: Some(self.root.path().join("absent-connections.toml")),
            home_dir: Some(self.root.path().join("home")),
            data_home: Some(self.data_home()),
            ..AppConfig::default()
        }
    }
}

fn pseudonym_seen_in(dir: &Path) -> String {
    open_pseudonymizer(dir, GeneratorConfig::default())
        .unwrap()
        .pseudonymize(Kind::Email, EMAIL)
        .text
}

fn info_warnings(contexts: &Contexts) -> Vec<Value> {
    let info = serde_json::to_value(server_info(
        contexts.column_policy_warnings(),
        contexts.deprecation_warnings().to_vec(),
    ))
    .unwrap();
    info["warnings"].as_array().cloned().unwrap_or_default()
}

fn data_directory_codes(warnings: &[Value]) -> Vec<String> {
    warnings
        .iter()
        .filter_map(|warning| warning["code"].as_str())
        .filter(|code| code.starts_with("data_directory_"))
        .map(str::to_string)
        .collect()
}

fn assert_translates_back(config: &AppConfig, pseudonym: &str) {
    let use_cases = AnonymizationInjector::from_config(config)
        .use_cases()
        .unwrap();
    assert_eq!(
        use_cases.translate_pseudonym.execute(pseudonym).real,
        Some((Kind::Email, EMAIL.to_string()))
    );
    assert_eq!(use_cases.anonymize_free_text.execute(EMAIL, &[]), pseudonym);
}

mod unit {
    use super::*;

    #[test]
    fn deux_dossiers_avec_un_secret_le_serveur_garde_l_ancien_et_signale_le_conflit() {
        let machine = Machine::new();
        let known = pseudonym_seen_in(&machine.legacy());
        pseudonym_seen_in(&machine.preferred());
        let config = machine.config(None);
        let contexts = Contexts::new(&config);
        let warnings = info_warnings(&contexts);
        assert_eq!(
            data_directory_codes(&warnings),
            vec!["data_directory_conflict"],
            "{warnings:?}"
        );
        let message = warnings
            .iter()
            .find(|warning| warning["code"] == "data_directory_conflict")
            .and_then(|warning| warning["message"].as_str())
            .unwrap()
            .to_string();
        assert!(
            message.contains(&machine.legacy().display().to_string()),
            "{message}"
        );
        assert!(
            message.contains(&machine.preferred().display().to_string()),
            "{message}"
        );
        let printed: Vec<_> = data_directory_notices(&config)
            .into_iter()
            .map(|notice| notice.code)
            .collect();
        assert_eq!(printed, vec!["data_directory_conflict"]);
        assert_translates_back(&config, &known);
    }

    #[test]
    fn un_nouveau_dossier_vide_ne_recoit_aucun_secret_au_demarrage() {
        let machine = Machine::new();
        let known = pseudonym_seen_in(&machine.legacy());
        fs::create_dir_all(machine.preferred()).unwrap();
        let config = machine.config(None);
        let _contexts = Contexts::new(&config);
        assert!(!machine.preferred().join(SECRET_FILE).exists());
        assert!(!machine.preferred().join(STORE_FILE).exists());
        assert_translates_back(&config, &known);
    }

    #[test]
    fn l_ancien_dossier_seul_reste_utilise_et_signale_la_migration() {
        let machine = Machine::new();
        let known = pseudonym_seen_in(&machine.legacy());
        let config = machine.config(None);
        let contexts = Contexts::new(&config);
        assert_eq!(
            data_directory_codes(&info_warnings(&contexts)),
            vec!["data_directory_deprecated"]
        );
        assert!(!machine.preferred().exists());
        assert_translates_back(&config, &known);
    }

    #[test]
    fn le_nouveau_dossier_seul_est_utilise_sans_avertissement_de_dossier() {
        let machine = Machine::new();
        let known = pseudonym_seen_in(&machine.preferred());
        let config = machine.config(None);
        let contexts = Contexts::new(&config);
        assert!(data_directory_codes(&info_warnings(&contexts)).is_empty());
        assert!(!machine.legacy().exists());
        assert_translates_back(&config, &known);
    }

    #[test]
    fn un_dossier_explicite_est_utilise_tel_quel_meme_face_a_deux_dossiers_par_defaut() {
        let machine = Machine::new();
        pseudonym_seen_in(&machine.legacy());
        pseudonym_seen_in(&machine.preferred());
        let explicit = machine.root.path().join("explicit");
        let known = pseudonym_seen_in(&explicit);
        let config = machine.config(Some(explicit));
        let contexts = Contexts::new(&config);
        assert!(data_directory_codes(&info_warnings(&contexts)).is_empty());
        assert_translates_back(&config, &known);
    }
}
