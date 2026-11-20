mod common;

use common::{
    CONNECTION, DescribeTableArgs, ExplainArgs, Harness, ListTablesArgs, QueryArgs,
    SearchSchemaArgs, body, describe_table, explain, is_error, list_tables, query, search_schema,
    text_of,
};
use serde_json::{Value, json};

const OTHER_DATABASE: &str = "postgres";

fn other() -> Option<String> {
    Some(OTHER_DATABASE.to_string())
}

async fn rows_of(harness: &Harness, sql: &str, database: Option<String>) -> Value {
    let result = query(
        &harness.state,
        QueryArgs {
            connection: CONNECTION.to_string(),
            sql: sql.to_string(),
            database,
            ..QueryArgs::default()
        },
    )
    .await;
    assert!(!is_error(&result), "{}", text_of(&result));
    body(&result)["rows"].clone()
}

mod e2e {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn query_atteint_une_seconde_base_du_meme_serveur() {
        let harness = Harness::new();
        let sql = "SELECT current_database() = 'postgres' AS on_postgres";
        assert_eq!(rows_of(&harness, sql, None).await, json!([[false]]));
        assert_eq!(rows_of(&harness, sql, other()).await, json!([[true]]));
    }

    #[tokio::test]
    #[ignore]
    async fn une_base_inconnue_rend_le_code_et_le_message_du_serveur() {
        let harness = Harness::new();
        let result = query(
            &harness.state,
            QueryArgs {
                connection: CONNECTION.to_string(),
                sql: "SELECT 1".to_string(),
                database: Some("absent_database_for_test".to_string()),
                ..QueryArgs::default()
            },
        )
        .await;
        assert!(is_error(&result));
        let text = text_of(&result);
        assert!(text.contains("3D000"), "{text}");
        assert!(text.contains("absent_database_for_test"), "{text}");
    }

    #[tokio::test]
    #[ignore]
    async fn explain_et_les_outils_de_schema_lisent_la_base_demandee() {
        let harness = Harness::new();

        let plan = explain(
            &harness.state,
            ExplainArgs {
                connection: CONNECTION.to_string(),
                sql: "SELECT 1".to_string(),
                database: other(),
            },
        )
        .await;
        assert!(!is_error(&plan), "{}", text_of(&plan));

        let default_tables = list_tables(
            &harness.state,
            ListTablesArgs {
                connection: CONNECTION.to_string(),
                schema: Some("public".to_string()),
                name_like: Some("customers".to_string()),
                limit: None,
                offset: None,
                database: None,
            },
        )
        .await;
        assert_eq!(body(&default_tables)["tables"].as_array().unwrap().len(), 1);
        let other_tables = list_tables(
            &harness.state,
            ListTablesArgs {
                connection: CONNECTION.to_string(),
                schema: Some("public".to_string()),
                name_like: Some("customers".to_string()),
                limit: None,
                offset: None,
                database: other(),
            },
        )
        .await;
        assert!(
            body(&other_tables)["tables"].as_array().unwrap().is_empty(),
            "{}",
            text_of(&other_tables)
        );

        let found = search_schema(
            &harness.state,
            SearchSchemaArgs {
                connection: CONNECTION.to_string(),
                pattern: "customers".to_string(),
                database: other(),
            },
        )
        .await;
        assert!(
            body(&found)["matches"].as_array().unwrap().is_empty(),
            "{}",
            text_of(&found)
        );

        let described = describe_table(
            &harness.state,
            DescribeTableArgs {
                connection: CONNECTION.to_string(),
                table: "customers".to_string(),
                database: other(),
            },
        )
        .await;
        assert!(is_error(&described));
        assert!(text_of(&described).contains("unknown_table"));
    }
}
