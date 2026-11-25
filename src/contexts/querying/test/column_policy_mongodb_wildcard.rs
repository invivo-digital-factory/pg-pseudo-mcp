use serde_json::json;

use super::column_policy_mongodb::{document, known_kind, merged, source};
use crate::contexts::querying::domain::services::column_policy::{field_hint, merge_policy};
use crate::contexts::querying::domain::services::document_clearances::{
    DocumentCommand, document_clearances,
};
use crate::contexts::querying::domain::services::mongo_guard::inspect_aggregate;
use crate::contexts::querying::domain::services::mongo_lineage::document_lineage;
use crate::contexts::querying::domain::types::{
    AggregateRequest, ColumnPolicy, DocumentClearance, PolicyDecision, QueryLimits, Reason,
    Translations,
};

fn personal(kind: &str) -> Option<PolicyDecision> {
    Some(PolicyDecision::Personal(kind.into()))
}

fn decided(policy: &ColumnPolicy, path: &str) -> Option<PolicyDecision> {
    policy
        .decide_field(&source("shop", "products", path))
        .cloned()
}

mod unit {
    use super::*;

    #[test]
    fn un_joker_couvre_un_seul_segment_de_n_importe_quel_nom() {
        let policy = merged(&[document(
            "columns.toml",
            &[("shop.products", "select_options.*.label")],
            &[],
        )]);
        for path in [
            "select_options.red.label",
            "select_options.A12-x.label",
            "select_options.*.label",
        ] {
            assert_eq!(
                decided(&policy, path),
                Some(PolicyDecision::Clear),
                "{path}"
            );
        }
        for path in [
            "select_options.label",
            "select_options.red.fr.label",
            "options.red.label",
            "select_options.red.title",
        ] {
            assert_eq!(decided(&policy, path), None, "{path}");
        }
    }

    #[test]
    fn un_joker_se_compare_sans_egard_a_la_casse() {
        let policy = merged(&[document(
            "columns.toml",
            &[("shop.products", "Select_Options.*.Label")],
            &[],
        )]);
        assert_eq!(
            decided(&policy, "SELECT_OPTIONS.Red.label"),
            Some(PolicyDecision::Clear)
        );
    }

    #[test]
    fn un_joker_en_clair_ne_couvre_ni_le_sous_arbre_ni_le_parent() {
        let policy = merged(&[document(
            "columns.toml",
            &[("shop.products", "select_options.*")],
            &[],
        )]);
        assert_eq!(
            decided(&policy, "select_options.red"),
            Some(PolicyDecision::Clear)
        );
        assert_eq!(decided(&policy, "select_options.red.label"), None);
        assert_eq!(decided(&policy, "select_options"), None);
    }

    #[test]
    fn des_jokers_bien_formes_sont_acceptes_en_clair_et_en_personnel() {
        for path in [
            "*",
            "*.label",
            "data.*",
            "data.*.*.labels",
            "data.units.*.labels.*",
        ] {
            let load = merge_policy(
                &[document(
                    "columns.toml",
                    &[("shop.products", path)],
                    &[("shop.customers", path, "email")],
                )],
                &known_kind,
            );
            assert!(load.warnings.is_empty(), "{path}: {:?}", load.warnings);
        }
    }

    #[test]
    fn un_joker_mal_forme_fait_ignorer_tout_le_fichier() {
        for path in [
            "select_options.a*.label",
            "select_options.*b.label",
            "select_options.**.label",
            "select_options.co*de.label",
            "**",
            "select_options.*.",
            "*.",
        ] {
            for (clear, personal) in [
                (vec![("shop.products", path)], vec![]),
                (vec![], vec![("shop.products", path, "email")]),
            ] {
                let mut clear = clear;
                clear.push(("*.products", "name"));
                let load = merge_policy(&[document("shared.toml", &clear, &personal)], &known_kind);
                assert_eq!(load.policy, ColumnPolicy::default(), "{path:?}");
                assert_eq!(load.warnings.len(), 1, "{path:?}");
                assert!(
                    load.warnings[0].contains("shared.toml")
                        && load.warnings[0].contains("is ignored"),
                    "{:?}",
                    load.warnings
                );
            }
        }
    }

    #[test]
    fn un_chemin_exact_l_emporte_sur_un_joker_dans_les_deux_sens() {
        let policy = merged(&[document(
            "columns.toml",
            &[
                ("shop.products", "select_options.*.label"),
                ("shop.products", "contact.main"),
            ],
            &[
                ("shop.products", "select_options.owner.label", "last_name"),
                ("shop.products", "contact.*", "email"),
            ],
        )]);
        assert_eq!(
            decided(&policy, "select_options.owner.label"),
            personal("last_name")
        );
        assert_eq!(
            decided(&policy, "select_options.red.label"),
            Some(PolicyDecision::Clear)
        );
        assert_eq!(
            decided(&policy, "contact.main"),
            Some(PolicyDecision::Clear)
        );
        assert_eq!(decided(&policy, "contact.other"), personal("email"));
    }

    #[test]
    fn le_chemin_qui_a_le_moins_de_jokers_l_emporte() {
        let policy = merged(&[document(
            "columns.toml",
            &[("shop.products", "data.units.*.labels")],
            &[("shop.products", "data.*.*.labels", "person_name")],
        )]);
        assert_eq!(
            decided(&policy, "data.units.kg.labels"),
            Some(PolicyDecision::Clear)
        );
        assert_eq!(
            decided(&policy, "data.sizes.xl.labels"),
            personal("person_name")
        );
    }

    #[test]
    fn a_precision_egale_le_personnel_l_emporte_sur_le_clair() {
        let policy = merged(&[document(
            "columns.toml",
            &[
                ("shop.products", "data.*.label"),
                ("shop.products", "*.red.title"),
            ],
            &[
                ("shop.products", "data.red.*", "person_name"),
                ("shop.products", "select_options.*.title", "last_name"),
            ],
        )]);
        assert_eq!(decided(&policy, "data.red.label"), personal("person_name"));
        assert_eq!(
            decided(&policy, "data.blue.label"),
            Some(PolicyDecision::Clear)
        );
        assert_eq!(
            decided(&policy, "select_options.red.title"),
            personal("last_name")
        );
    }

    #[test]
    fn a_precision_egale_le_personnel_l_emporte_aussi_entre_deux_fichiers() {
        for (shared, local) in [
            (
                document(
                    "shared.toml",
                    &[],
                    &[("shop.products", "data.*.label", "person_name")],
                ),
                document("local.toml", &[("shop.products", "data.red.*")], &[]),
            ),
            (
                document("shared.toml", &[("shop.products", "data.red.*")], &[]),
                document(
                    "local.toml",
                    &[],
                    &[("shop.products", "data.*.label", "person_name")],
                ),
            ),
        ] {
            let policy = merged(&[shared, local]);
            assert_eq!(decided(&policy, "data.red.label"), personal("person_name"));
        }
    }

    #[test]
    fn deux_types_personnels_a_precision_egale_donnent_toujours_le_meme_choix() {
        let first = decided(
            &merged(&[document(
                "columns.toml",
                &[],
                &[
                    ("shop.products", "data.*.label", "email"),
                    ("shop.products", "data.red.*", "phone"),
                ],
            )]),
            "data.red.label",
        );
        assert!(
            matches!(first, Some(PolicyDecision::Personal(_))),
            "{first:?}"
        );
        for _ in 0..32 {
            let again = decided(
                &merged(&[document(
                    "columns.toml",
                    &[],
                    &[
                        ("shop.products", "data.red.*", "phone"),
                        ("shop.products", "data.*.label", "email"),
                    ],
                )]),
                "data.red.label",
            );
            assert_eq!(again, first);
        }
    }

    #[test]
    fn la_cle_de_collection_passe_avant_la_precision_du_chemin() {
        let policy = merged(&[document(
            "columns.toml",
            &[("shop.products", "select_options.*.label")],
            &[("*.products", "select_options.red.label", "last_name")],
        )]);
        assert_eq!(
            decided(&policy, "select_options.red.label"),
            Some(PolicyDecision::Clear)
        );
        assert_eq!(
            policy
                .decide_field(&source("crm", "products", "select_options.red.label"))
                .cloned(),
            personal("last_name")
        );
    }

    #[test]
    fn le_meme_joker_clair_et_personnel_dans_un_fichier_le_fait_ignorer() {
        let load = merge_policy(
            &[document(
                "shared.toml",
                &[("shop.products", "select_options.*.label")],
                &[("shop.products", "Select_Options.*.LABEL", "last_name")],
            )],
            &known_kind,
        );
        assert_eq!(load.policy, ColumnPolicy::default());
        assert_eq!(load.warnings.len(), 1, "{:?}", load.warnings);
    }

    #[test]
    fn le_fichier_local_remplace_le_meme_joker_du_fichier_partage() {
        let policy = merged(&[
            document(
                "shared.toml",
                &[],
                &[("shop.products", "select_options.*.label", "last_name")],
            ),
            document(
                "local.toml",
                &[("shop.products", "select_options.*.label")],
                &[],
            ),
        ]);
        assert_eq!(
            decided(&policy, "select_options.red.label"),
            Some(PolicyDecision::Clear)
        );
    }

    #[test]
    fn un_champ_couvert_par_un_joker_ne_recoit_pas_d_indice() {
        let policy = merged(&[document(
            "columns.toml",
            &[("shop.products", "select_options.*.label")],
            &[("shop.products", "select_options.*.owner", "person_name")],
        )]);
        for path in ["select_options.red.label", "select_options.red.owner"] {
            let field = source("shop", "products", path);
            assert_eq!(
                field_hint(&policy, &field, Reason::Dictionary, "last_name"),
                None,
                "{path}"
            );
            assert_eq!(
                field_hint(&policy, &field, Reason::Default, "text"),
                None,
                "{path}"
            );
        }
        assert!(
            field_hint(
                &policy,
                &source("shop", "products", "select_options.red.title"),
                Reason::Default,
                "text"
            )
            .is_some()
        );
    }

    #[test]
    fn un_personnel_a_joker_sous_un_champ_l_empeche_d_etre_prouve_en_clair() {
        let policy = merged(&[document(
            "columns.toml",
            &[
                ("shop.products", "contact"),
                ("shop.products", "select_options"),
                ("shop.products", "status"),
            ],
            &[
                ("shop.products", "contact.*", "email"),
                ("shop.products", "*.*.owner", "person_name"),
            ],
        )]);
        let below = |path: &str| policy.has_personal_field_below(&source("shop", "products", path));
        assert!(below("contact"));
        assert!(below("select_options"));
        assert!(below("select_options.red"));
        assert!(!below("select_options.red.owner"));
        assert!(below("status"));
        assert!(!policy.has_personal_field_below(&source("shop", "orders", "contact")));
    }

    #[test]
    fn un_joker_seul_ne_couvre_jamais_la_racine_d_un_document() {
        let policy = merged(&[document("columns.toml", &[("*.*", "*")], &[])]);
        assert_eq!(decided(&policy, ""), None);
        assert_eq!(decided(&policy, "status"), Some(PolicyDecision::Clear));
    }

    #[test]
    fn un_document_joint_entier_n_est_pas_prouve_en_clair_par_un_joker_seul() {
        let policy = merged(&[document("columns.toml", &[("*.*", "*")], &[])]);
        let guarded = inspect_aggregate(&AggregateRequest {
            database: Some("shop".into()),
            collection: "orders".into(),
            pipeline: vec![
                json!({"$lookup": {"from": "customers", "localField": "buyer", "foreignField": "_id", "as": "buyer"}}),
                json!({"$project": {"copy": {"$ifNull": ["$buyer", 0]}, "buyer": 1}}),
            ],
            ..AggregateRequest::default()
        })
        .expect("allowed")
        .finish(&Translations::default(), &QueryLimits::default());
        let lineage = document_lineage(&guarded, "shop");
        let observed = [vec!["copy".to_string()], vec!["buyer".to_string()]]
            .into_iter()
            .collect();
        let clearances = document_clearances(
            &observed,
            &lineage,
            &policy,
            ("shop", "orders"),
            DocumentCommand::Aggregate,
        );
        assert!(
            clearances.contains(&(
                vec!["copy".to_string()],
                DocumentClearance::Derived {
                    proven_clear: false
                }
            )),
            "{clearances:?}"
        );
        assert!(
            !clearances.contains(&(vec!["buyer".to_string()], DocumentClearance::Clear)),
            "{clearances:?}"
        );
    }

    #[test]
    fn un_personnel_de_la_collection_est_toujours_sous_la_racine() {
        let policy = merged(&[document(
            "columns.toml",
            &[],
            &[("shop.products", "contact.email", "email")],
        )]);
        assert!(policy.has_personal_field_below(&source("shop", "products", "")));
    }
}
