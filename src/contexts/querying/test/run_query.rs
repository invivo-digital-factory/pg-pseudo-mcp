use std::sync::Arc;

use super::fakes::{
    FakeAnonymizer, FakeDatabase, FakePolicy, FakeTranslator, empty_output, ports, sql_error,
};
use crate::contexts::querying::domain::errors::{DatabaseFailure, QueryingError};
use crate::contexts::querying::domain::services::column_policy::merge_policy;
use crate::contexts::querying::domain::types::Clearance::{
    ClearByPolicy, PersonalByPolicy, ProvenClear, Unproven,
};
use crate::contexts::querying::domain::types::{
    Cell, ColumnFlag, ColumnMeta, ColumnPolicyDocument, ColumnPolicyEntries, ColumnSource,
    QueryOutput, Reason,
};
use crate::contexts::querying::domain::types::{ExplainRequest, QueryRequest};
use crate::contexts::querying::use_cases::{ExplainQuery, RunQuery};

fn request(sql: &str) -> QueryRequest {
    QueryRequest {
        connection: "Shop".into(),
        sql: sql.into(),
        ..QueryRequest::default()
    }
}

mod unit {
    use super::*;

    #[tokio::test]
    async fn une_requete_refusee_n_atteint_jamais_la_base() {
        let database = FakeDatabase::returning(empty_output());
        let use_case = RunQuery::new(
            Arc::clone(&database),
            ports(FakeTranslator::default(), Arc::default()),
        );

        let error = use_case
            .execute(request("DELETE FROM orders"))
            .await
            .unwrap_err();

        match error {
            QueryingError::Refused(refusal) => assert_eq!(refusal.code, "statement_not_allowed"),
            other => panic!("{other:?}"),
        }
        assert!(database.statements().is_empty());
    }

    #[tokio::test]
    async fn un_registre_inutilisable_refuse_avant_d_appeler_la_base() {
        let database = FakeDatabase::returning(empty_output());
        let use_case = RunQuery::new(Arc::clone(&database), Err("store is locked".into()));

        let error = use_case
            .execute(request("SELECT id FROM orders"))
            .await
            .unwrap_err();

        assert_eq!(
            error,
            QueryingError::StoreUnavailable {
                reason: "store is locked".into()
            }
        );
        assert!(database.statements().is_empty());
    }

    #[tokio::test]
    async fn un_registre_inutilisable_refuse_aussi_un_explain_avant_la_base() {
        let database = FakeDatabase::returning(empty_output());
        let use_case = ExplainQuery::new(Arc::clone(&database), Err("store is locked".into()));

        let error = use_case
            .execute(ExplainRequest {
                connection: "Shop".into(),
                database: None,
                sql: "SELECT 1".into(),
            })
            .await
            .unwrap_err();

        assert!(matches!(error, QueryingError::StoreUnavailable { .. }));
        assert!(database.statements().is_empty());
    }

    #[tokio::test]
    async fn le_litteral_traduit_est_celui_envoye_a_la_base() {
        let database = FakeDatabase::returning(empty_output());
        let translator =
            FakeTranslator::default().with("anne.lorin7@example.net", "anne@canary-mail.fr");
        let use_case = RunQuery::new(Arc::clone(&database), ports(translator, Arc::default()));

        let response = use_case
            .execute(request(
                "SELECT id FROM customers WHERE email = 'anne.lorin7@example.net'",
            ))
            .await
            .unwrap();

        let statements = database.statements();
        assert_eq!(statements.len(), 1);
        assert!(
            statements[0].contains("'anne@canary-mail.fr'"),
            "{}",
            statements[0]
        );
        assert!(!statements[0].contains("example.net"), "{}", statements[0]);
        assert_eq!(response.translated_literals, 1);
        assert!(response.warnings.is_empty());
    }

    #[tokio::test]
    async fn un_litteral_inconnu_en_forme_de_pseudonyme_part_tel_quel_avec_avertissement() {
        let database = FakeDatabase::returning(empty_output());
        let use_case = RunQuery::new(
            Arc::clone(&database),
            ports(FakeTranslator::default(), Arc::default()),
        );

        let response = use_case
            .execute(request(
                "SELECT id FROM customers WHERE email = 'nobody.known@example.net'",
            ))
            .await
            .unwrap();

        assert!(database.statements()[0].contains("nobody.known@example.net"));
        assert_eq!(response.translated_literals, 0);
        assert_eq!(response.warnings.len(), 1);
        assert_eq!(response.warnings[0].code, "unresolved_pseudonym");
    }

    #[tokio::test]
    async fn une_erreur_de_base_citant_une_valeur_reelle_revient_avec_son_pseudonyme() {
        let database = FakeDatabase::failing(sql_error("invalid input syntax near \"Dupont\""));
        let anonymizer: Arc<FakeAnonymizer> = Arc::default();
        let translator = FakeTranslator::default().with("Kalin Torvo", "Dupont");
        let use_case = RunQuery::new(
            Arc::clone(&database),
            ports(translator, Arc::clone(&anonymizer)),
        );

        let error = use_case
            .execute(request(
                "SELECT id FROM customers WHERE last_name = 'Kalin Torvo'",
            ))
            .await
            .unwrap_err();

        match error {
            QueryingError::SqlError { message, .. } => {
                assert!(message.contains("Kalin Torvo"), "{message}");
                assert!(!message.contains("Dupont"), "{message}");
            }
            other => panic!("{other:?}"),
        }
        assert!(
            anonymizer
                .injected_seen()
                .contains(&vec![("Dupont".to_string(), "Kalin Torvo".to_string())])
        );
    }

    #[tokio::test]
    async fn un_echec_de_connexion_citant_une_valeur_reelle_revient_avec_son_pseudonyme() {
        let database = FakeDatabase::failing(DatabaseFailure::ConnectFailed {
            connection: "Shop".into(),
            reason: "SQLSTATE 28P01: password authentication failed for user \"Dupont\"".into(),
        });
        let translator = FakeTranslator::default().with("Kalin Torvo", "Dupont");
        let use_case = RunQuery::new(Arc::clone(&database), ports(translator, Arc::default()));

        let error = use_case
            .execute(request(
                "SELECT id FROM customers WHERE last_name = 'Kalin Torvo'",
            ))
            .await
            .unwrap_err();

        match error {
            QueryingError::ConnectFailed { reason, .. } => {
                assert!(reason.contains("Kalin Torvo"), "{reason}");
                assert!(!reason.contains("Dupont"), "{reason}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn les_libelles_d_enum_de_la_base_sont_transmis_a_l_anonymiseur() {
        let labels = vec!["AUTHORIZED".to_string(), "SEPA".to_string()];
        let database = FakeDatabase::returning_with_enum_labels(empty_output(), Ok(labels.clone()));
        let anonymizer: Arc<FakeAnonymizer> = Arc::default();
        let use_case = RunQuery::new(
            Arc::clone(&database),
            ports(FakeTranslator::default(), Arc::clone(&anonymizer)),
        );

        use_case
            .execute(request("SELECT status::text FROM payment"))
            .await
            .unwrap();

        assert_eq!(anonymizer.enum_labels_seen(), vec![labels]);
    }

    #[tokio::test]
    async fn une_lecture_des_libelles_d_enum_en_echec_n_empeche_pas_la_requete() {
        let database = FakeDatabase::returning_with_enum_labels(
            empty_output(),
            Err(sql_error("permission denied for table pg_enum")),
        );
        let anonymizer: Arc<FakeAnonymizer> = Arc::default();
        let use_case = RunQuery::new(
            Arc::clone(&database),
            ports(FakeTranslator::default(), Arc::clone(&anonymizer)),
        );

        let response = use_case
            .execute(request("SELECT status::text FROM payment"))
            .await
            .unwrap();

        assert!(response.warnings.is_empty());
        assert_eq!(anonymizer.enum_labels_seen(), vec![Vec::<String>::new()]);
    }

    fn output(columns: Vec<ColumnMeta>, rows: Vec<Vec<Cell>>) -> QueryOutput {
        QueryOutput {
            columns,
            rows,
            more_rows: false,
            notices: Vec::new(),
        }
    }

    fn text_column(name: &str, source: Option<&str>) -> ColumnMeta {
        ColumnMeta {
            name: name.into(),
            type_name: "text".into(),
            source: source.map(|column| ColumnSource {
                schema: "public".into(),
                table: "orders".into(),
                column: column.into(),
            }),
        }
    }

    fn orders_catalog() -> QueryOutput {
        let row = |name: &str, type_name: &str| {
            vec![
                Cell::Text(name.into()),
                Cell::Text(type_name.into()),
                Cell::Text("public".into()),
                Cell::Text("orders".into()),
            ]
        };
        output(
            Vec::new(),
            vec![
                row("id", "int4"),
                row("status", "enum"),
                row("payment_method", "enum"),
                row("titulaire", "text"),
            ],
        )
    }

    fn policy(clear: &[&str], personal: &[(&str, &str)]) -> Arc<FakePolicy> {
        let document = ColumnPolicyDocument {
            file: "columns.toml".into(),
            content: Ok(ColumnPolicyEntries {
                clear: clear.iter().map(|item| item.to_string()).collect(),
                personal: personal
                    .iter()
                    .map(|(pattern, kind)| (pattern.to_string(), kind.to_string()))
                    .collect(),
                ..ColumnPolicyEntries::default()
            }),
        };
        Arc::new(FakePolicy(merge_policy(&[document], &|_| true)))
    }

    #[tokio::test]
    async fn la_politique_de_colonnes_est_transmise_a_l_anonymiseur_pour_les_colonnes_lues() {
        let database = FakeDatabase::returning_with_catalog(
            output(
                vec![
                    text_column("titulaire", Some("titulaire")),
                    text_column("reference", Some("reference")),
                    text_column("label", Some("label")),
                ],
                vec![vec![
                    Cell::Text("Garance".into()),
                    Cell::Text("lot-42".into()),
                    Cell::Text("x".into()),
                ]],
            ),
            orders_catalog(),
        );
        let anonymizer: Arc<FakeAnonymizer> = Arc::default();
        let use_case = RunQuery::new(
            Arc::clone(&database),
            ports(FakeTranslator::default(), Arc::clone(&anonymizer)),
        )
        .with_column_policy(policy(
            &["*.titulaire"],
            &[("public.orders.reference", "last_name")],
        ));

        use_case
            .execute(request("SELECT titulaire, reference, label FROM orders"))
            .await
            .unwrap();

        assert_eq!(
            anonymizer.clearances_seen(),
            vec![vec![
                ClearByPolicy,
                PersonalByPolicy("last_name".into()),
                Unproven
            ]]
        );
    }

    #[tokio::test]
    async fn une_expression_sur_une_colonne_declaree_en_clair_est_prouvee_par_la_politique() {
        let database = FakeDatabase::returning_with_catalog(
            output(
                vec![text_column("etiquette", None), text_column("brut", None)],
                vec![vec![
                    Cell::Text("Garance paid".into()),
                    Cell::Text("paid - CASH".into()),
                ]],
            ),
            orders_catalog(),
        );
        let anonymizer: Arc<FakeAnonymizer> = Arc::default();
        let use_case = RunQuery::new(
            Arc::clone(&database),
            ports(FakeTranslator::default(), Arc::clone(&anonymizer)),
        )
        .with_column_policy(policy(&["orders.titulaire"], &[]));

        use_case
            .execute(request(
                "SELECT concat(titulaire, ' ', status) AS etiquette, concat(status, ' - ', payment_method) AS brut FROM orders",
            ))
            .await
            .unwrap();

        assert_eq!(
            anonymizer.clearances_seen(),
            vec![vec![ClearByPolicy, ProvenClear]]
        );
    }

    #[tokio::test]
    async fn une_colonne_non_classee_pseudonymisee_par_le_dictionnaire_porte_un_indice() {
        let database = FakeDatabase::returning(output(
            vec![
                text_column("nom", Some("nom")),
                text_column("titulaire", Some("titulaire")),
            ],
            vec![vec![Cell::Text("a".into()), Cell::Text("b".into())]],
        ));
        let flag = |kind: &str, why| {
            Some(ColumnFlag {
                kind: kind.into(),
                why,
            })
        };
        let anonymizer = FakeAnonymizer::flagging(vec![
            flag("last_name", Reason::Dictionary),
            flag("person_name", Reason::Default),
        ]);
        let use_case = RunQuery::new(
            Arc::clone(&database),
            ports(FakeTranslator::default(), anonymizer),
        )
        .with_column_policy(policy(&["orders.titulaire"], &[]));

        let response = use_case
            .execute(request("SELECT nom, titulaire FROM orders"))
            .await
            .unwrap();

        let hints = serde_json::to_value(&response.column_policy_hints).unwrap();
        assert_eq!(
            hints,
            serde_json::json!([{
                "column": "public.orders.nom",
                "reason": "dictionary",
                "hint": "classify it in the column policy: clear or personal"
            }])
        );
    }

    #[tokio::test]
    async fn une_colonne_derivee_de_colonnes_en_clair_est_signalee_prouvee_a_l_anonymiseur() {
        let database = FakeDatabase::returning_with_catalog(
            output(
                vec![
                    text_column("etiquette", None),
                    text_column("titulaire", Some("titulaire")),
                    text_column("nom", None),
                ],
                vec![vec![
                    Cell::Text("paid - CASH".into()),
                    Cell::Text("Ylwenna Kervadec".into()),
                    Cell::Text("YLWENNA KERVADEC".into()),
                ]],
            ),
            orders_catalog(),
        );
        let anonymizer: Arc<FakeAnonymizer> = Arc::default();
        let use_case = RunQuery::new(
            Arc::clone(&database),
            ports(FakeTranslator::default(), Arc::clone(&anonymizer)),
        );

        use_case
            .execute(request(
                "SELECT concat(status, ' - ', payment_method) AS etiquette, titulaire, upper(titulaire) AS nom FROM orders",
            ))
            .await
            .unwrap();

        assert_eq!(
            anonymizer.clearances_seen(),
            vec![vec![ProvenClear, Unproven, Unproven]]
        );
    }

    #[tokio::test]
    async fn un_catalogue_illisible_laisse_les_colonnes_derivees_au_jugement_habituel() {
        let database = FakeDatabase::returning_with_enum_labels(
            output(
                vec![text_column("etiquette", None)],
                vec![vec![Cell::Text("paid - CASH".into())]],
            ),
            Ok(Vec::new()),
        );
        let anonymizer: Arc<FakeAnonymizer> = Arc::default();
        let use_case = RunQuery::new(
            Arc::clone(&database),
            ports(FakeTranslator::default(), Arc::clone(&anonymizer)),
        );

        use_case
            .execute(request(
                "SELECT concat(status, ' - ', payment_method) AS etiquette FROM orders",
            ))
            .await
            .unwrap();

        assert_eq!(anonymizer.clearances_seen(), vec![vec![Unproven]]);
    }
}
