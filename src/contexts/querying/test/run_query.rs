use std::sync::Arc;

use super::fakes::{FakeAnonymizer, FakeDatabase, FakeTranslator, empty_output, ports, sql_error};
use crate::contexts::querying::domain::errors::QueryingError;
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
}
