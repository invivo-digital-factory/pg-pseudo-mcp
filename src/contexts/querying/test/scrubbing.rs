use std::sync::Arc;

use super::fakes::{FakeAnonymizer, FakeDatabase, FakeTranslator, ports, sql_error};
use crate::contexts::querying::domain::errors::DatabaseFailure;
use crate::contexts::querying::domain::types::{
    DescribeTableRequest, ExplainRequest, ListTablesRequest, SearchSchemaRequest,
};
use crate::contexts::querying::use_cases::{DescribeTable, ExplainQuery, ListTables, SearchSchema};

const REAL: &str = "Dupont";
const PSEUDONYM: &str = "Moreau";

async fn error_texts(database: Arc<FakeDatabase>) -> Vec<(&'static str, String)> {
    let anonymizer = || {
        ports(
            FakeTranslator::default(),
            FakeAnonymizer::masking(REAL, PSEUDONYM),
        )
    };
    let connection = || "Shop".to_string();
    vec![
        (
            "explain_query",
            ExplainQuery::new(Arc::clone(&database), anonymizer())
                .execute(ExplainRequest {
                    connection: connection(),
                    database: None,
                    sql: "SELECT 1".into(),
                })
                .await
                .unwrap_err()
                .to_string(),
        ),
        (
            "list_tables",
            ListTables::new(Arc::clone(&database), anonymizer())
                .execute(ListTablesRequest {
                    connection: connection(),
                    database: None,
                    schema: None,
                    name_like: None,
                    limit: None,
                    offset: None,
                })
                .await
                .unwrap_err()
                .to_string(),
        ),
        (
            "describe_table",
            DescribeTable::new(Arc::clone(&database), anonymizer())
                .execute(DescribeTableRequest {
                    connection: connection(),
                    database: None,
                    table: "users".into(),
                })
                .await
                .unwrap_err()
                .to_string(),
        ),
        (
            "search_schema",
            SearchSchema::new(Arc::clone(&database), anonymizer())
                .execute(SearchSchemaRequest {
                    connection: connection(),
                    database: None,
                    pattern: "user".into(),
                })
                .await
                .unwrap_err()
                .to_string(),
        ),
    ]
}

mod unit {
    use super::*;

    #[tokio::test]
    async fn aucun_cas_d_usage_du_catalogue_ne_laisse_fuir_un_nom_reel_dans_ses_erreurs() {
        let failures = [
            (
                "echec de connexion",
                DatabaseFailure::ConnectFailed {
                    connection: "Shop".into(),
                    reason: format!("SQLSTATE 28000: no access for {REAL}"),
                },
            ),
            (
                "erreur de base",
                sql_error(&format!("duplicate key for {REAL}")),
            ),
            (
                "connexion perdue",
                DatabaseFailure::Connect(format!("connection lost for {REAL}")),
            ),
        ];

        for (label, failure) in failures {
            for (use_case, text) in error_texts(FakeDatabase::failing(failure)).await {
                assert!(!text.contains(REAL), "{use_case} / {label}: {text}");
                assert!(text.contains(PSEUDONYM), "{use_case} / {label}: {text}");
            }
        }
    }
}
