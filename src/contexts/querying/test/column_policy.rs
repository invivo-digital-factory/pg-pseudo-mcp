use crate::contexts::querying::domain::services::column_policy::{
    combine_lineage, describe_hint, merge_policy, parse_pattern, policy_clearances, result_hints,
};
use crate::contexts::querying::domain::services::column_pseudonymization::policy_pseudonymization;
use crate::contexts::querying::domain::types::Clearance::{
    ClearByPolicy, PersonalByPolicy, ProvenClear, Unproven,
};
use crate::contexts::querying::domain::types::{
    ColumnClassification, ColumnEntry, ColumnFlag, ColumnMeta, ColumnPattern, ColumnPolicy,
    ColumnPolicyDocument, ColumnPolicyEntries, ColumnPolicyHint, ColumnSource, PolicyDecision,
    Reason,
};

fn source(schema: &str, table: &str, column: &str) -> ColumnSource {
    ColumnSource {
        schema: schema.into(),
        table: table.into(),
        column: column.into(),
    }
}

fn document(file: &str, clear: &[&str], personal: &[(&str, &str)]) -> ColumnPolicyDocument {
    ColumnPolicyDocument {
        file: file.into(),
        content: Ok(ColumnPolicyEntries {
            clear: clear.iter().map(|item| item.to_string()).collect(),
            personal: personal
                .iter()
                .map(|(pattern, kind)| (pattern.to_string(), kind.to_string()))
                .collect(),
        }),
    }
}

fn known_kind(kind: &str) -> bool {
    ["person_name", "last_name", "siret", "email"].contains(&kind)
}

fn merged(documents: &[ColumnPolicyDocument]) -> ColumnPolicy {
    let load = merge_policy(documents, &known_kind);
    assert!(load.warnings.is_empty(), "{:?}", load.warnings);
    load.policy
}

fn column(name: &str, table_column: Option<(&str, &str)>) -> ColumnMeta {
    ColumnMeta {
        name: name.into(),
        type_name: "text".into(),
        source: table_column.map(|(table, column)| source("public", table, column)),
    }
}

fn flag(kind: &str, why: Reason) -> Option<ColumnFlag> {
    Some(ColumnFlag {
        kind: kind.into(),
        why,
    })
}

mod unit {
    use super::*;

    #[test]
    fn les_trois_formes_de_motif_sont_reconnues() {
        assert_eq!(
            parse_pattern("public.payment.carrier_name"),
            Ok(ColumnPattern {
                schema: Some("public".into()),
                table: Some("payment".into()),
                column: "carrier_name".into(),
            })
        );
        assert_eq!(
            parse_pattern("orders.reference"),
            Ok(ColumnPattern {
                schema: None,
                table: Some("orders".into()),
                column: "reference".into(),
            })
        );
        assert_eq!(
            parse_pattern("*.Product_Name"),
            Ok(ColumnPattern {
                schema: None,
                table: None,
                column: "product_name".into(),
            })
        );
    }

    #[test]
    fn un_motif_mal_forme_est_refuse() {
        for text in [
            "product_name",
            "",
            "a.b.c.d",
            "*.*",
            "orders.*",
            "*.orders.reference",
            "public..reference",
            " .x",
        ] {
            assert!(parse_pattern(text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn le_motif_le_plus_precis_l_emporte() {
        let policy = merged(&[document(
            "shared.toml",
            &["*.product_name", "public.catalog.product_name"],
            &[("catalog.product_name", "last_name")],
        )]);
        assert_eq!(
            policy.decide(&source("public", "catalog", "product_name")),
            Some(&PolicyDecision::Clear)
        );
        assert_eq!(
            policy.decide(&source("sales", "catalog", "product_name")),
            Some(&PolicyDecision::Personal("last_name".into()))
        );
        assert_eq!(
            policy.decide(&source("sales", "orders", "product_name")),
            Some(&PolicyDecision::Clear)
        );
        assert_eq!(policy.decide(&source("sales", "orders", "label")), None);
    }

    #[test]
    fn la_personnalisation_d_un_schema_table_colonne_l_emporte_sur_l_etoile() {
        let policy = merged(&[document(
            "shared.toml",
            &["*.product_name"],
            &[("public.products.product_name", "last_name")],
        )]);
        assert_eq!(
            policy.decide(&source("public", "products", "product_name")),
            Some(&PolicyDecision::Personal("last_name".into()))
        );
    }

    #[test]
    fn le_fichier_local_l_emporte_sur_le_fichier_partage_pour_la_meme_cle() {
        let policy = merged(&[
            document(
                "shared.toml",
                &["orders.reference"],
                &[("*.product_name", "last_name")],
            ),
            document(
                "local.toml",
                &["*.product_name"],
                &[("orders.reference", "siret")],
            ),
        ]);
        assert_eq!(
            policy.decide(&source("public", "products", "product_name")),
            Some(&PolicyDecision::Clear)
        );
        assert_eq!(
            policy.decide(&source("public", "orders", "reference")),
            Some(&PolicyDecision::Personal("siret".into()))
        );
    }

    #[test]
    fn la_correspondance_ignore_la_casse_des_noms() {
        let policy = merged(&[document("shared.toml", &["Public.Orders.Reference"], &[])]);
        assert_eq!(
            policy.decide(&source("public", "orders", "REFERENCE")),
            Some(&PolicyDecision::Clear)
        );
    }

    #[test]
    fn un_fichier_invalide_est_ignore_avec_un_avertissement_qui_le_nomme() {
        let load = merge_policy(
            &[
                document("shared.toml", &["*.product_name"], &[]),
                ColumnPolicyDocument {
                    file: "local.toml".into(),
                    content: Err("syntax error at line 2, column 5".into()),
                },
            ],
            &known_kind,
        );
        assert_eq!(
            load.policy
                .decide(&source("public", "products", "product_name")),
            Some(&PolicyDecision::Clear)
        );
        assert_eq!(load.warnings.len(), 1);
        assert!(
            load.warnings[0].contains("local.toml"),
            "{:?}",
            load.warnings
        );
        assert!(
            load.warnings[0].contains("syntax error at line 2, column 5"),
            "{:?}",
            load.warnings
        );
    }

    #[test]
    fn un_type_inconnu_fait_ignorer_tout_le_fichier() {
        let load = merge_policy(
            &[document(
                "shared.toml",
                &["*.product_name"],
                &[("customer.contact", "telephone_portable")],
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
        assert!(
            load.warnings[0].contains("telephone_portable"),
            "{:?}",
            load.warnings
        );
    }

    #[test]
    fn un_motif_invalide_fait_ignorer_tout_le_fichier() {
        let load = merge_policy(
            &[document(
                "shared.toml",
                &["*.product_name", "product_name"],
                &[],
            )],
            &known_kind,
        );
        assert_eq!(load.policy, ColumnPolicy::default());
        assert_eq!(load.warnings.len(), 1);
    }

    #[test]
    fn une_colonne_a_la_fois_claire_et_personnelle_dans_un_fichier_le_rend_invalide() {
        let load = merge_policy(
            &[document(
                "shared.toml",
                &["*.contact"],
                &[("*.contact", "person_name")],
            )],
            &known_kind,
        );
        assert_eq!(load.policy, ColumnPolicy::default());
        assert_eq!(load.warnings.len(), 1);
    }

    #[test]
    fn la_lignee_prouvee_par_la_politique_seule_est_marquee_comme_telle() {
        assert_eq!(
            combine_lineage(
                vec![ProvenClear, Unproven, Unproven],
                vec![ProvenClear, ProvenClear, Unproven],
            ),
            vec![ProvenClear, ClearByPolicy, Unproven]
        );
    }

    #[test]
    fn la_politique_s_applique_aux_colonnes_lues_directement_et_pas_aux_alias() {
        let policy = merged(&[document(
            "shared.toml",
            &["*.product_name"],
            &[("orders.reference", "siret")],
        )]);
        let columns = [
            column("product_name", Some(("products", "product_name"))),
            column("reference", Some(("orders", "reference"))),
            column("product_name", None),
            column("label", Some(("orders", "label"))),
        ];
        assert_eq!(
            policy_clearances(
                &columns,
                &policy,
                vec![Unproven, Unproven, Unproven, Unproven]
            ),
            vec![
                ClearByPolicy,
                PersonalByPolicy("siret".into()),
                Unproven,
                Unproven
            ]
        );
    }

    #[test]
    fn un_indice_est_donne_pour_chaque_colonne_source_non_classee_pseudonymisee_par_dictionnaire_ou_defaut()
     {
        let policy = merged(&[document("shared.toml", &["*.product_name"], &[])]);
        let columns = [
            column("nom", Some(("customers", "nom"))),
            column("titulaire", Some(("orders", "titulaire"))),
            column("email", Some(("customers", "email"))),
            column("product_name", Some(("products", "product_name"))),
            column("v", None),
            column("ville", Some(("customers", "ville"))),
            column("nom", Some(("customers", "nom"))),
        ];
        let flags = [
            flag("last_name", Reason::Dictionary),
            flag("person_name", Reason::Default),
            flag("email", Reason::Detected),
            flag("email", Reason::Detected),
            flag("person_name", Reason::Derived),
            None,
            flag("last_name", Reason::Dictionary),
        ];
        assert_eq!(
            result_hints(&columns, &flags, &policy),
            vec![
                ColumnPolicyHint {
                    column: "public.customers.nom".into(),
                    reason: Reason::Dictionary,
                    hint: "classify it in the column policy: clear or personal",
                },
                ColumnPolicyHint {
                    column: "public.orders.titulaire".into(),
                    reason: Reason::Default,
                    hint: "classify it in the column policy: clear or personal",
                },
            ]
        );
    }

    #[test]
    fn une_colonne_classee_ne_recoit_plus_d_indice() {
        let policy = merged(&[document(
            "shared.toml",
            &["customers.nom"],
            &[("orders.titulaire", "person_name")],
        )]);
        let columns = [
            column("nom", Some(("customers", "nom"))),
            column("titulaire", Some(("orders", "titulaire"))),
        ];
        let flags = [
            flag("last_name", Reason::Dictionary),
            flag("person_name", Reason::Policy),
        ];
        assert!(result_hints(&columns, &flags, &policy).is_empty());
    }
    fn classification(forced_kind: Option<&str>, clear_by_type: bool) -> ColumnClassification {
        ColumnClassification {
            forced_kind: forced_kind.map(str::to_string),
            clear_by_type,
        }
    }

    #[test]
    fn la_description_suit_la_politique_de_colonnes() {
        let dictionary = classification(Some("last_name"), false);
        let heuristic = classification(None, false);
        assert_eq!(
            policy_pseudonymization(&dictionary, Some(&PolicyDecision::Clear), "text"),
            None
        );
        assert_eq!(
            policy_pseudonymization(&heuristic, Some(&PolicyDecision::Clear), "varchar(40)"),
            None
        );
        assert_eq!(
            policy_pseudonymization(&heuristic, Some(&PolicyDecision::Clear), "inet").as_deref(),
            Some("ip_address")
        );
        assert_eq!(
            policy_pseudonymization(
                &heuristic,
                Some(&PolicyDecision::Personal("siret".into())),
                "text"
            )
            .as_deref(),
            Some("siret")
        );
        assert_eq!(
            policy_pseudonymization(&dictionary, None, "text").as_deref(),
            Some("last_name")
        );
    }

    fn entry(pseudonymized: Option<&str>, policy: Option<&'static str>) -> ColumnEntry {
        ColumnEntry {
            name: "c".into(),
            type_name: "text".into(),
            not_null: false,
            pseudonymized: pseudonymized.map(str::to_string),
            policy,
            comment: None,
        }
    }

    #[test]
    fn la_description_signale_les_colonnes_a_classer() {
        let hint = |class: &ColumnClassification, entry: &ColumnEntry| {
            describe_hint("public.t.c".into(), class, entry).map(|hint| hint.reason)
        };
        let dictionary = classification(Some("last_name"), false);
        let heuristic = classification(None, false);
        assert_eq!(
            hint(&dictionary, &entry(Some("last_name"), None)),
            Some(Reason::Dictionary)
        );
        assert_eq!(
            hint(&heuristic, &entry(Some("detected_per_value"), None)),
            Some(Reason::Default)
        );
        assert_eq!(hint(&heuristic, &entry(Some("ip_address"), None)), None);
        assert_eq!(hint(&heuristic, &entry(None, None)), None);
        assert_eq!(hint(&dictionary, &entry(None, Some("clear"))), None);
        assert_eq!(
            hint(&heuristic, &entry(Some("siret"), Some("personal"))),
            None
        );
    }
}
