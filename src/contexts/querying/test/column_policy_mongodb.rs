use crate::contexts::querying::domain::services::column_policy::{field_hint, merge_policy};
use crate::contexts::querying::domain::types::{
    ColumnPolicy, ColumnPolicyDocument, ColumnPolicyEntries, FieldPolicyHint, FieldSource,
    MongoPolicyEntries, PolicyDecision, Reason,
};

fn source(database: &str, collection: &str, path: &str) -> FieldSource {
    FieldSource {
        database: database.into(),
        collection: collection.into(),
        path: path.into(),
    }
}

fn document(
    file: &str,
    clear: &[(&str, &str)],
    personal: &[(&str, &str, &str)],
) -> ColumnPolicyDocument {
    ColumnPolicyDocument {
        file: file.into(),
        content: Ok(ColumnPolicyEntries {
            mongodb: MongoPolicyEntries {
                clear: clear
                    .iter()
                    .map(|(key, path)| (key.to_string(), path.to_string()))
                    .collect(),
                personal: personal
                    .iter()
                    .map(|(key, path, kind)| (key.to_string(), path.to_string(), kind.to_string()))
                    .collect(),
            },
            ..ColumnPolicyEntries::default()
        }),
    }
}

fn known_kind(kind: &str) -> bool {
    ["person_name", "last_name", "phone", "email"].contains(&kind)
}

fn merged(documents: &[ColumnPolicyDocument]) -> ColumnPolicy {
    let load = merge_policy(documents, &known_kind);
    assert!(load.warnings.is_empty(), "{:?}", load.warnings);
    load.policy
}

mod unit {
    use super::*;

    #[test]
    fn une_entree_claire_de_toutes_les_bases_rend_le_champ_en_clair_sans_indice() {
        let policy = merged(&[document("columns.toml", &[("*.products", "name")], &[])]);
        let field = source("shop", "products", "name");
        assert_eq!(policy.decide_field(&field), Some(&PolicyDecision::Clear));
        assert_eq!(
            field_hint(&policy, &field, Reason::Dictionary, "last_name"),
            None
        );
    }

    #[test]
    fn sans_entree_un_champ_de_nom_du_dictionnaire_recoit_un_indice_au_format_mongodb() {
        let policy = ColumnPolicy::default();
        let field = source("shop", "products", "name");
        assert_eq!(policy.decide_field(&field), None);
        assert_eq!(
            field_hint(&policy, &field, Reason::Dictionary, "last_name"),
            Some(FieldPolicyHint {
                collection: "shop.products".into(),
                field: "name".into(),
                reason: Reason::Dictionary,
                hint: "classify it in the column policy: clear or personal",
            })
        );
    }

    #[test]
    fn l_indice_mongodb_se_serialise_avec_collection_champ_raison_et_indice() {
        let hint = field_hint(
            &ColumnPolicy::default(),
            &source("shop", "orders.archive", "buyer.name"),
            Reason::Default,
            "text",
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(hint).unwrap(),
            serde_json::json!({
                "collection": "shop.orders.archive",
                "field": "buyer.name",
                "reason": "default",
                "hint": "classify it in the column policy: clear or personal",
            })
        );
    }

    #[test]
    fn les_indices_mongodb_suivent_les_regles_des_indices_de_colonnes() {
        let policy = ColumnPolicy::default();
        let field = source("shop", "customers", "email");
        assert!(field_hint(&policy, &field, Reason::Dictionary, "email").is_none());
        assert!(field_hint(&policy, &field, Reason::Dictionary, "person_name").is_some());
        assert!(field_hint(&policy, &field, Reason::Default, "text").is_some());
        assert!(field_hint(&policy, &field, Reason::Detected, "email").is_none());
        assert!(field_hint(&policy, &field, Reason::Policy, "email").is_none());
    }

    #[test]
    fn un_champ_deja_classe_ne_recoit_pas_d_indice() {
        let policy = merged(&[document(
            "columns.toml",
            &[],
            &[("shop.customers", "name", "last_name")],
        )]);
        assert_eq!(
            field_hint(
                &policy,
                &source("shop", "customers", "name"),
                Reason::Dictionary,
                "last_name"
            ),
            None
        );
    }

    #[test]
    fn la_cle_se_coupe_au_premier_point_la_collection_peut_en_contenir() {
        let policy = merged(&[document(
            "columns.toml",
            &[("shop.orders.archive", "status")],
            &[],
        )]);
        assert_eq!(
            policy.decide_field(&source("shop", "orders.archive", "status")),
            Some(&PolicyDecision::Clear)
        );
        assert_eq!(
            policy.decide_field(&source("shop.orders", "archive", "status")),
            None
        );
        assert_eq!(
            policy.decide_field(&source("shop", "orders", "status")),
            None
        );
    }

    #[test]
    fn la_correspondance_du_chemin_ignore_la_casse_et_exige_le_chemin_entier() {
        let policy = merged(&[document(
            "columns.toml",
            &[("shop.orders", "Items.Label")],
            &[],
        )]);
        assert_eq!(
            policy.decide_field(&source("shop", "orders", "items.label")),
            Some(&PolicyDecision::Clear)
        );
        assert_eq!(
            policy.decide_field(&source("shop", "orders", "items")),
            None
        );
        assert_eq!(
            policy.decide_field(&source("shop", "orders", "items.label.fr")),
            None
        );
    }

    #[test]
    fn les_noms_de_base_et_de_collection_se_comparent_exactement() {
        let policy = merged(&[document("columns.toml", &[("shop.orders", "status")], &[])]);
        assert_eq!(
            policy.decide_field(&source("Shop", "orders", "status")),
            None
        );
        assert_eq!(
            policy.decide_field(&source("shop", "Orders", "status")),
            None
        );
    }

    #[test]
    fn la_precedence_va_de_base_collection_a_etoile_etoile() {
        let policy = merged(&[document(
            "columns.toml",
            &[("*.*", "name")],
            &[
                ("shop.orders", "name", "person_name"),
                ("*.orders", "name", "last_name"),
                ("shop.*", "name", "phone"),
            ],
        )]);
        let decide = |database: &str, collection: &str| {
            policy
                .decide_field(&source(database, collection, "name"))
                .cloned()
        };
        assert_eq!(
            decide("shop", "orders"),
            Some(PolicyDecision::Personal("person_name".into()))
        );
        assert_eq!(
            decide("crm", "orders"),
            Some(PolicyDecision::Personal("last_name".into()))
        );
        assert_eq!(
            decide("shop", "customers"),
            Some(PolicyDecision::Personal("phone".into()))
        );
        assert_eq!(decide("crm", "customers"), Some(PolicyDecision::Clear));
    }

    #[test]
    fn une_collection_precise_l_emporte_sur_une_base_entiere_pour_le_meme_chemin() {
        let policy = merged(&[document(
            "columns.toml",
            &[("shop.orders", "name")],
            &[("shop.*", "name", "phone")],
        )]);
        assert_eq!(
            policy.decide_field(&source("shop", "orders", "name")),
            Some(&PolicyDecision::Clear)
        );
    }

    #[test]
    fn le_fichier_local_l_emporte_sur_le_fichier_partage_pour_la_meme_cle_mongodb() {
        let policy = merged(&[
            document(
                "shared.toml",
                &[("shop.orders", "status")],
                &[("shop.customers", "contact.mobile", "phone")],
            ),
            document(
                "local.toml",
                &[("shop.customers", "contact.mobile")],
                &[("shop.orders", "status", "person_name")],
            ),
        ]);
        assert_eq!(
            policy.decide_field(&source("shop", "customers", "contact.mobile")),
            Some(&PolicyDecision::Clear)
        );
        assert_eq!(
            policy.decide_field(&source("shop", "orders", "status")),
            Some(&PolicyDecision::Personal("person_name".into()))
        );
    }

    #[test]
    fn le_meme_chemin_clair_et_personnel_dans_un_fichier_le_fait_ignorer() {
        let load = merge_policy(
            &[document(
                "shared.toml",
                &[("shop.orders", "status"), ("*.products", "name")],
                &[("shop.orders", "Status", "person_name")],
            )],
            &known_kind,
        );
        assert_eq!(load.policy, ColumnPolicy::default());
        assert_eq!(load.warnings.len(), 1);
        assert!(
            load.warnings[0].contains("shared.toml"),
            "{:?}",
            load.warnings
        );
    }

    #[test]
    fn un_type_inconnu_dans_mongodb_personal_fait_ignorer_le_fichier() {
        let load = merge_policy(
            &[document(
                "shared.toml",
                &[("*.products", "name")],
                &[("shop.customers", "contact.mobile", "telephone_portable")],
            )],
            &known_kind,
        );
        assert_eq!(load.policy, ColumnPolicy::default());
        assert_eq!(load.warnings.len(), 1);
        assert!(
            load.warnings[0].contains("telephone_portable"),
            "{:?}",
            load.warnings
        );
    }

    #[test]
    fn une_cle_ou_un_chemin_mal_formes_font_ignorer_le_fichier() {
        for (key, path) in [
            ("orders", "status"),
            ("", "status"),
            (".orders", "status"),
            ("shop.", "status"),
            ("shop.orders", ""),
            ("shop.orders", "items..label"),
            ("shop.orders", ".items"),
        ] {
            let load = merge_policy(&[document("shared.toml", &[(key, path)], &[])], &known_kind);
            assert_eq!(load.policy, ColumnPolicy::default(), "{key:?} {path:?}");
            assert_eq!(load.warnings.len(), 1, "{key:?} {path:?}");
        }
    }

    #[test]
    fn un_fichier_valide_garde_ses_entrees_postgresql_et_mongodb() {
        let mut valid = document("shared.toml", &[("*.products", "name")], &[]);
        if let Ok(entries) = valid.content.as_mut() {
            entries.clear.push("*.product_name".into());
        }
        let load = merge_policy(
            &[
                valid,
                document(
                    "local.toml",
                    &[("*.products", "name")],
                    &[("*.products", "name", "unknown")],
                ),
            ],
            &known_kind,
        );
        assert_eq!(load.warnings.len(), 1, "{:?}", load.warnings);
        assert_eq!(
            load.policy
                .decide_field(&source("shop", "products", "name")),
            Some(&PolicyDecision::Clear)
        );
        assert_eq!(
            load.policy
                .decide(&crate::contexts::querying::domain::types::ColumnSource {
                    schema: "public".into(),
                    table: "products".into(),
                    column: "product_name".into(),
                }),
            Some(&PolicyDecision::Clear)
        );
    }
}
