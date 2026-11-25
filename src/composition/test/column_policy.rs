use std::fs;
use std::path::Path;

use serde_json::Value;
use tempfile::TempDir;

use crate::common::AppConfig;
use crate::composition::contexts::Contexts;
use crate::composition::server_info::server_info;
use crate::contexts::anonymization::domain::types::{
    FieldClearance, FieldPath, Kind, Node, Reason as Why,
};
use crate::contexts::anonymization::injector::AnonymizationInjector;
use crate::contexts::connections::presentation::list_connections_with_warnings;
use crate::contexts::querying::domain::services::column_policy::field_hint;
use crate::contexts::querying::domain::types::{FieldSource, PolicyDecision, Reason};

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

    fn reason_of(why: Why) -> Reason {
        match why {
            Why::Dictionary => Reason::Dictionary,
            Why::Default => Reason::Default,
            _ => Reason::Detected,
        }
    }

    fn products_name_after(dir: &Path, shared: &str) -> (Node, Vec<String>, bool) {
        let contexts = contexts(dir, shared, None);
        let policy = contexts.querying.read_column_policy.execute().policy;
        let field = FieldSource {
            database: "shop".into(),
            collection: "products".into(),
            path: "name".into(),
        };
        let clearances: Vec<(FieldPath, FieldClearance)> = match policy.decide_field(&field) {
            Some(PolicyDecision::Clear) => vec![(FieldPath::new("name"), FieldClearance::Clear)],
            Some(PolicyDecision::Personal(kind)) => vec![(
                FieldPath::new("name"),
                FieldClearance::Personal(Kind::parse(kind).unwrap()),
            )],
            None => Vec::new(),
        };
        let document = Node::Object(vec![("name".into(), Node::Text("Garance Bleue".into()))]);
        let anonymized = AnonymizationInjector::new(&dir.join("data"))
            .use_cases()
            .unwrap()
            .anonymize_documents
            .execute(vec![document], &clearances, &[]);
        let hinted = anonymized.pseudonymized.get("name").and_then(|flag| {
            field_hint(&policy, &field, reason_of(flag.why), &flag.kind)
                .map(|hint| format!("{}:{}:{:?}", hint.collection, hint.field, hint.reason))
        });
        (
            anonymized.documents[0].clone(),
            hinted.into_iter().collect(),
            anonymized.pseudonymized.contains_key("name"),
        )
    }

    #[test]
    fn ae12_un_champ_declare_en_clair_dans_le_fichier_revient_en_clair_sans_indice() {
        let dir = TempDir::new().unwrap();
        let (document, hints, flagged) =
            products_name_after(dir.path(), "[mongodb.clear]\n\"*.products\" = [\"name\"]");
        assert_eq!(
            document,
            Node::Object(vec![("name".into(), Node::Text("Garance Bleue".into()))])
        );
        assert!(hints.is_empty(), "{hints:?}");
        assert!(!flagged);
    }

    #[test]
    fn ae12_sans_entree_le_champ_est_pseudonymise_comme_nom_de_famille_avec_un_indice() {
        let dir = TempDir::new().unwrap();
        let (document, hints, flagged) = products_name_after(dir.path(), "");
        assert_ne!(
            document,
            Node::Object(vec![("name".into(), Node::Text("Garance Bleue".into()))])
        );
        assert_eq!(hints, ["shop.products:name:Dictionary"]);
        assert!(flagged);
    }

    #[test]
    fn un_joker_mal_forme_fait_ignorer_le_fichier_qui_le_porte_avec_un_avertissement() {
        let dir = TempDir::new().unwrap();
        let contexts = contexts(
            dir.path(),
            "[mongodb.clear]\n\"shop.attributes\" = [\"select_options.*.label\", \"data.opt*.label\"]",
            Some("[mongodb.clear]\n\"*.products\" = [\"name\"]"),
        );
        let warnings = contexts.column_policy_warnings();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("team-columns.toml"), "{warnings:?}");
        assert!(warnings[0].contains("wildcard"), "{warnings:?}");
        let policy = contexts.querying.read_column_policy.execute().policy;
        let field = |collection: &str, path: &str| FieldSource {
            database: "shop".into(),
            collection: collection.into(),
            path: path.into(),
        };
        assert_eq!(
            policy.decide_field(&field("attributes", "select_options.red.label")),
            None
        );
        assert_eq!(
            policy.decide_field(&field("products", "name")),
            Some(&PolicyDecision::Clear)
        );
    }
}
