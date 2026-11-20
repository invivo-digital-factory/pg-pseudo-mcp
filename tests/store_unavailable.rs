mod common;

use common::{
    DescribeTableArgs, ExplainArgs, ListTablesArgs, QueryArgs, SearchSchemaArgs, body,
    describe_table, explain, is_error, list_connections, list_tables, query, search_schema,
};
use pg_pseudo_mcp::common::AppConfig;
use pg_pseudo_mcp::composition::contexts::Contexts;
use rmcp::model::CallToolResult;

fn contexts_without_secret() -> (Contexts, tempfile::TempDir, tempfile::TempDir) {
    let dbeaver = tempfile::tempdir().expect("dbeaver dir");
    let data = tempfile::tempdir().expect("data dir");
    std::fs::write(data.path().join("mapping.sqlite"), b"").expect("store file");
    let config = AppConfig {
        dbeaver_home: Some(dbeaver.path().to_path_buf()),
        data_dir: Some(data.path().to_path_buf()),
        ssh_known_hosts: None,
    };
    (Contexts::new(&config), dbeaver, data)
}

fn assert_refused_fail_closed(result: &CallToolResult) {
    assert!(is_error(result));
    let error = &body(result)["error"];
    assert_eq!(error["code"], "pseudonym_store_unavailable");
    assert_eq!(error["category"], "withheld_fail_closed");
    assert_eq!(error["retryable"], false);
}

mod e2e {
    use super::*;

    #[tokio::test]
    async fn un_secret_absent_garde_le_serveur_actif_mais_refuse_tous_les_outils_de_donnees() {
        let (contexts, _dbeaver, _data) = contexts_without_secret();

        let connections = list_connections(&contexts).await;
        assert!(!is_error(&connections));
        assert!(body(&connections)["connections"].is_array());

        assert_refused_fail_closed(
            &query(
                &contexts,
                QueryArgs {
                    connection: "any".into(),
                    sql: "SELECT 1".into(),
                    ..QueryArgs::default()
                },
            )
            .await,
        );
        assert_refused_fail_closed(
            &explain(
                &contexts,
                ExplainArgs {
                    connection: "any".into(),
                    database: None,
                    sql: "SELECT 1".into(),
                },
            )
            .await,
        );
        assert_refused_fail_closed(
            &search_schema(
                &contexts,
                SearchSchemaArgs {
                    connection: "any".into(),
                    database: None,
                    pattern: "user".into(),
                },
            )
            .await,
        );
        assert_refused_fail_closed(
            &list_tables(
                &contexts,
                ListTablesArgs {
                    connection: "any".into(),
                    database: None,
                    schema: None,
                    name_like: None,
                    limit: None,
                    offset: None,
                },
            )
            .await,
        );
        assert_refused_fail_closed(
            &describe_table(
                &contexts,
                DescribeTableArgs {
                    connection: "any".into(),
                    database: None,
                    table: "users".into(),
                },
            )
            .await,
        );
    }
}
