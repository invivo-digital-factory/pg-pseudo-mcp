use std::fs;
use std::path::Path;

use serde_json::Value;
use tempfile::TempDir;

use crate::common::AppConfig;
use crate::composition::contexts::Contexts;
use crate::composition::server_info::server_info;
use crate::contexts::anonymization::injector::AnonymizationInjector;
use crate::contexts::connections::presentation::list_connections_with_warnings;

fn contexts(dir: &Path, shared: &str, local: Option<&str>) -> Contexts {
    let shared_path = dir.join("team-columns.toml");
    fs::write(&shared_path, shared).unwrap();
    let home = dir.join("home");
    if let Some(local) = local {
        let folder = home.join(".config").join("scarif");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("columns.toml"), local).unwrap();
    }
    let ssh_config = dir.join("ssh_config");
    fs::write(&ssh_config, "").unwrap();
    let config = AppConfig {
        dbeaver_home: Some(dir.join("dbeaver")),
        data_dir: Some(dir.join("data")),
        ssh_config: Some(ssh_config),
        connections_file: Some(dir.join("absent-connections.toml")),
        columns_file: Some(shared_path),
        home_dir: Some(home),
        ..AppConfig::default()
    };
    let anonymization = AnonymizationInjector::new(&dir.join("data")).use_cases();
    Contexts::with_anonymization(&config, anonymization)
}

fn body(result: &rmcp::model::CallToolResult) -> Value {
    let text = serde_json::to_value(result).unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    serde_json::from_str(&text).unwrap()
}

mod unit {
    use super::*;

    #[test]
    fn des_fichiers_valides_ne_produisent_aucun_avertissement() {
        let dir = TempDir::new().unwrap();
        let contexts = contexts(
            dir.path(),
            "clear = [\"*.product_name\"]",
            Some("[personal]\n\"customer.contact\" = \"person_name\""),
        );
        assert!(contexts.column_policy_warnings().is_empty());
    }

    #[test]
    fn chaque_fichier_invalide_est_nomme_dans_un_avertissement_sans_valeur() {
        let dir = TempDir::new().unwrap();
        let contexts = contexts(
            dir.path(),
            "[personal]\n\"customer.contact\" = \"telephone_portable\"",
            Some("clear = [\n"),
        );
        let warnings = contexts.column_policy_warnings();
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0].contains("team-columns.toml"), "{warnings:?}");
        assert!(warnings[0].contains("telephone_portable"), "{warnings:?}");
        assert!(warnings[1].contains("columns.toml"), "{warnings:?}");
        assert!(warnings[1].contains("line"), "{warnings:?}");
    }

    #[test]
    fn server_info_porte_les_avertissements_de_politique() {
        let info = serde_json::to_value(server_info(
            vec!["policy file x ignored".into()],
            Vec::new(),
        ))
        .unwrap();
        assert_eq!(
            info["warnings"],
            serde_json::json!([{ "code": "column_policy_ignored", "message": "policy file x ignored" }])
        );
        let quiet = serde_json::to_value(server_info(Vec::new(), Vec::new())).unwrap();
        assert!(quiet.get("warnings").is_none());
    }

    #[tokio::test]
    async fn la_liste_des_connexions_porte_les_avertissements_de_politique() {
        let dir = TempDir::new().unwrap();
        let contexts = contexts(dir.path(), "clear = 1", None);
        let warnings = contexts.column_policy_warnings();
        let result =
            list_connections_with_warnings(&contexts.connections.list_connections, &warnings).await;
        let body = body(&result);
        assert_eq!(body["warnings"][0]["code"], "column_policy_ignored");
        assert!(
            body["warnings"][0]["message"]
                .as_str()
                .unwrap()
                .contains("team-columns.toml"),
            "{body}"
        );
        assert!(body["connections"].is_array(), "{body}");
    }

    #[test]
    fn un_fichier_de_colonnes_de_l_ancien_dossier_est_lu_et_signale() {
        let dir = TempDir::new().unwrap();
        let home = dir.path().join("home");
        let folder = home.join(".config").join("pg-pseudo");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("columns.toml"), "clear = [\"*.product_name\"]").unwrap();
        fs::write(folder.join("connections.toml"), "").unwrap();
        let config = AppConfig {
            data_dir: Some(dir.path().join("data")),
            home_dir: Some(home.clone()),
            ..AppConfig::default()
        };
        let contexts = Contexts::with_anonymization(
            &config,
            AnonymizationInjector::new(&dir.path().join("data")).use_cases(),
        );
        assert!(contexts.column_policy_warnings().is_empty());
        let info = serde_json::to_value(server_info(
            contexts.column_policy_warnings(),
            contexts.deprecation_warnings().to_vec(),
        ))
        .unwrap();
        let warnings = info["warnings"].as_array().unwrap();
        assert_eq!(warnings.len(), 2, "{info}");
        assert!(
            warnings
                .iter()
                .all(|w| w["code"] == "config_location_deprecated"),
            "{info}"
        );
        let text = info["warnings"].to_string();
        assert!(
            text.contains("columns.toml") && text.contains("connections.toml"),
            "{info}"
        );
        assert!(text.contains(".config/scarif"), "{info}");
        assert!(!home.join(".config/scarif").exists());
    }

    #[test]
    fn aucun_avertissement_de_deplacement_quand_le_nouveau_dossier_porte_le_fichier() {
        let dir = TempDir::new().unwrap();
        let contexts = contexts(dir.path(), "clear = []", Some("clear = []"));
        assert!(contexts.deprecation_warnings().is_empty());
    }
}
