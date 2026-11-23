use std::fs;

use tempfile::TempDir;

use crate::contexts::connections::domain::repositories::ColumnPolicyFiles;
use crate::contexts::connections::domain::types::ColumnPolicyEntries;
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
