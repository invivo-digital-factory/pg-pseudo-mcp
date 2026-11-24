use std::fs;

use tempfile::TempDir;

use crate::contexts::connections::domain::repositories::ColumnPolicyFiles;
use crate::contexts::connections::domain::types::{ColumnPolicyEntries, MongoPolicyEntries};
use crate::contexts::connections::infrastructure::data_sources::column_policy_file::parse_policy;
use crate::contexts::connections::infrastructure::repositories::TomlColumnPolicyFiles;

const POLICY: &str = r#"
clear = ["*.product_name", "public.payment.carrier_name", "orders.reference"]

[personal]
"customer.contact" = "person_name"
"*.siret_client" = "siret"
"#;

mod unit {
    use super::*;

    #[test]
    fn le_fichier_de_politique_donne_les_colonnes_claires_et_personnelles() {
        assert_eq!(
            parse_policy(POLICY),
            Ok(ColumnPolicyEntries {
                clear: vec![
                    "*.product_name".to_string(),
                    "public.payment.carrier_name".to_string(),
                    "orders.reference".to_string(),
                ],
                personal: vec![
                    ("*.siret_client".to_string(), "siret".to_string()),
                    ("customer.contact".to_string(), "person_name".to_string()),
                ],
                ..ColumnPolicyEntries::default()
            })
        );
    }

    #[test]
    fn un_fichier_vide_ne_declare_rien() {
        assert_eq!(parse_policy(""), Ok(ColumnPolicyEntries::default()));
    }

    #[test]
    fn une_erreur_de_syntaxe_donne_la_position_sans_citer_le_contenu() {
        let error = parse_policy("clear = [\"*.product_name\"\n[personal\n").unwrap_err();
        assert!(error.contains("line 2"), "{error}");
        assert!(!error.contains("product_name"), "{error}");
        assert!(!error.contains("personal"), "{error}");
    }

    #[test]
    fn une_structure_inattendue_est_refusee() {
        for contents in [
            "clear = \"*.product_name\"",
            "clear = [1]",
            "personal = [\"*.contact\"]",
            "[personal]\n\"*.contact\" = 1",
            "secret = \"x\"",
        ] {
            assert!(parse_policy(contents).is_err(), "{contents}");
        }
    }

    #[test]
    fn la_section_mongodb_donne_les_champs_clairs_et_personnels_par_collection() {
        let contents = r#"
clear = ["*.product_name"]

[mongodb.clear]
"shop.orders" = ["items.label", "status"]
"*.products" = ["name"]

[mongodb.personal."shop.customers"]
"contact.mobile" = "phone"
"#;
        assert_eq!(
            parse_policy(contents),
            Ok(ColumnPolicyEntries {
                clear: vec!["*.product_name".to_string()],
                personal: Vec::new(),
                mongodb: MongoPolicyEntries {
                    clear: vec![
                        ("*.products".to_string(), "name".to_string()),
                        ("shop.orders".to_string(), "items.label".to_string()),
                        ("shop.orders".to_string(), "status".to_string()),
                    ],
                    personal: vec![(
                        "shop.customers".to_string(),
                        "contact.mobile".to_string(),
                        "phone".to_string()
                    )],
                },
            })
        );
    }

    #[test]
    fn une_section_mongodb_mal_structuree_est_refusee() {
        for contents in [
            "mongodb = 1",
            "[mongodb]\nclear = [\"name\"]",
            "[mongodb]\nsecret = {}",
            "[mongodb.clear]\n\"shop.orders\" = \"status\"",
            "[mongodb.clear]\n\"shop.orders\" = [1]",
            "[mongodb.personal]\n\"shop.customers\" = \"phone\"",
            "[mongodb.personal.\"shop.customers\"]\n\"contact.mobile\" = 1",
        ] {
            assert!(parse_policy(contents).is_err(), "{contents}");
        }
    }

    #[test]
    fn le_fichier_partage_est_lu_avant_le_fichier_local() {
        let dir = TempDir::new().unwrap();
        let shared = dir.path().join("shared.toml");
        let local = dir.path().join("local.toml");
        fs::write(&shared, "clear = [\"*.product_name\"]").unwrap();
        fs::write(&local, "clear = [\"orders.reference\"]").unwrap();
        let files = TomlColumnPolicyFiles::new(Some(shared), Some(local)).read();
        assert_eq!(files.len(), 2);
        assert!(files[0].file.ends_with("shared.toml"), "{}", files[0].file);
        assert!(files[1].file.ends_with("local.toml"), "{}", files[1].file);
        assert_eq!(
            files[1].content,
            Ok(ColumnPolicyEntries {
                clear: vec!["orders.reference".to_string()],
                personal: Vec::new(),
                ..ColumnPolicyEntries::default()
            })
        );
    }

    #[test]
    fn un_fichier_local_absent_est_ignore_sans_avertissement() {
        let dir = TempDir::new().unwrap();
        let files = TomlColumnPolicyFiles::new(None, Some(dir.path().join("absent.toml"))).read();
        assert!(files.is_empty());
    }

    #[test]
    fn un_fichier_partage_configure_mais_absent_est_signale() {
        let dir = TempDir::new().unwrap();
        let files = TomlColumnPolicyFiles::new(Some(dir.path().join("absent.toml")), None).read();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].content, Err("the file does not exist".to_string()));
    }
}
