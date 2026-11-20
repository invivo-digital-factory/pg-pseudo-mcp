mod common;

use common::{
    CONNECTION, Harness, ListDatabasesArgs, QueryArgs, assert_no_canary, body, is_error,
    list_databases, query, text_of,
};
use serde_json::Value;

fn default_database() -> String {
    common::database_url()
        .rsplit('/')
        .next()
        .expect("database name")
        .to_string()
}

async fn listed(harness: &Harness) -> Value {
    let result = list_databases(
        &harness.state,
        ListDatabasesArgs {
            connection: CONNECTION.to_string(),
        },
    )
    .await;
    assert!(!is_error(&result), "{}", text_of(&result));
    assert_no_canary(&text_of(&result));
    body(&result)
}

mod e2e {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn la_liste_contient_la_base_de_test_et_marque_celle_de_la_connexion() {
        let harness = Harness::new();
        let response = listed(&harness).await;
        let names: Vec<&str> = response["databases"]
            .as_array()
            .expect("databases array")
            .iter()
            .map(|name| name.as_str().expect("name"))
            .collect();
        assert!(names.contains(&default_database().as_str()), "{response}");
        assert!(names.contains(&"postgres"), "{response}");
        assert!(!names.iter().any(|name| name.starts_with("template")));
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
        assert_eq!(response["default"], default_database());
    }

    #[tokio::test]
    #[ignore]
    async fn pg_database_reste_refuse_dans_le_sql_libre() {
        let harness = Harness::new();
        let result = query(
            &harness.state,
            QueryArgs {
                connection: CONNECTION.to_string(),
                sql: "SELECT datname FROM pg_database".to_string(),
                ..QueryArgs::default()
            },
        )
        .await;
        assert!(is_error(&result));
        assert_eq!(body(&result)["error"]["category"], "refused_read_only");
    }

    #[tokio::test]
    #[ignore]
    async fn une_connexion_inconnue_est_signalee() {
        let harness = Harness::new();
        let result = list_databases(
            &harness.state,
            ListDatabasesArgs {
                connection: "absent".to_string(),
            },
        )
        .await;
        assert!(is_error(&result));
        assert_eq!(body(&result)["error"]["code"], "unknown_connection");
    }

    #[tokio::test]
    #[ignore]
    async fn un_echec_de_connexion_passe_par_la_pseudonymisation() {
        let harness = Harness::with_login("marie.dupont@yourcompany.fr", "hunter2-wrong");
        let result = list_databases(
            &harness.state,
            ListDatabasesArgs {
                connection: CONNECTION.to_string(),
            },
        )
        .await;
        assert!(is_error(&result));
        let text = text_of(&result);
        assert!(text.contains("28P01"), "{text}");
        assert!(!text.contains("marie.dupont@yourcompany.fr"), "{text}");
        assert!(!text.contains("hunter2-wrong"), "{text}");
    }
}
