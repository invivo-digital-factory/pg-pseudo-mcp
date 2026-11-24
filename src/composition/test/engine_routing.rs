use std::fs;

use serde_json::{Value, json};
use tempfile::TempDir;

use crate::common::AppConfig;
use crate::composition::contexts::Contexts;
use crate::composition::tools;
use crate::contexts::anonymization::injector::AnonymizationInjector;
use crate::contexts::querying::presentation::{
    AggregateArgs, ExplainArgs, FindArgs, ListTablesArgs, QueryArgs,
};

const MONGO: &str = "atlas";
const POSTGRES: &str = "shop";

fn contexts(dir: &TempDir) -> Contexts {
    let root = dir.path();
    let marker = root.join("command-ran");
    let connections = root.join("connections.toml");
    fs::write(
        &connections,
        format!(
            "[[connection]]\nname = \"{MONGO}\"\nengine = \"mongodb\"\nhost = \"pl-0.atlas.scarif.test\"\nsrv = false\nport = 1024\ntls = false\nuser = \"reader\"\ntype = \"test\"\nauth = \"command\"\npassword_command = [\"/usr/bin/touch\", \"{}\"]\n\n[[connection]]\nname = \"atlas-admin\"\nengine = \"mongodb\"\nhost = \"pl-0.atlas.scarif.test\"\nsrv = false\nport = 1024\ntls = false\ndatabase = \"admin\"\nuser = \"reader\"\ntype = \"test\"\nauth = \"command\"\npassword_command = [\"/usr/bin/touch\", \"{}\"]\n\n[[connection]]\nname = \"{POSTGRES}\"\nhost = \"db.internal.example\"\ndatabase = \"shop\"\nuser = \"reader\"\ntype = \"dev\"\nauth = \"command\"\npassword_command = [\"/usr/bin/touch\", \"{}\"]\n",
            marker.display(),
            marker.display(),
            marker.display()
        ),
    )
    .unwrap();
    let ssh_config = root.join("ssh_config");
    fs::write(&ssh_config, "").unwrap();
    let config = AppConfig {
        dbeaver_home: Some(root.join("dbeaver")),
        data_dir: Some(root.join("data")),
        ssh_config: Some(ssh_config),
        connections_file: Some(connections),
        ..AppConfig::default()
    };
    Contexts::with_anonymization(
        &config,
        AnonymizationInjector::new(&root.join("data")).use_cases(),
    )
}

fn error_of(result: &rmcp::model::CallToolResult) -> Value {
    assert_eq!(result.is_error, Some(true));
    let text = serde_json::to_value(result).unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    serde_json::from_str::<Value>(&text).unwrap()["error"].clone()
}

mod unit {
    use super::*;

    #[tokio::test]
    async fn ae1_query_et_explain_sur_mongodb_renvoient_vers_find_ou_aggregate() {
        let dir = TempDir::new().unwrap();
        let contexts = contexts(&dir);
        for result in [
            tools::query(
                &contexts,
                QueryArgs {
                    connection: MONGO.into(),
                    sql: "SELECT 1".into(),
                    ..QueryArgs::default()
                },
            )
            .await,
            tools::explain(
                &contexts,
                ExplainArgs {
                    connection: MONGO.into(),
                    sql: "SELECT 1".into(),
                    database: None,
                },
            )
            .await,
        ] {
            let error = error_of(&result);
            assert_eq!(error["code"], "wrong_engine");
            let message = error["message"].as_str().unwrap();
            assert!(
                message.contains("find") && message.contains("aggregate"),
                "{message}"
            );
        }
        assert!(!dir.path().join("command-ran").exists());
    }

    #[tokio::test]
    async fn ae1_find_et_aggregate_sur_postgresql_renvoient_vers_query() {
        let dir = TempDir::new().unwrap();
        let contexts = contexts(&dir);
        for result in [
            tools::find(
                &contexts,
                FindArgs {
                    connection: POSTGRES.into(),
                    collection: "customers".into(),
                    filter: Some(json!({})),
                    ..FindArgs::default()
                },
            )
            .await,
            tools::aggregate(
                &contexts,
                AggregateArgs {
                    connection: POSTGRES.into(),
                    collection: "customers".into(),
                    pipeline: vec![json!({"$match": {}})],
                    ..AggregateArgs::default()
                },
            )
            .await,
        ] {
            let error = error_of(&result);
            assert_eq!(error["code"], "wrong_engine");
            assert!(
                error["message"].as_str().unwrap().contains("query"),
                "{error}"
            );
        }
        assert!(!dir.path().join("command-ran").exists());
    }

    #[tokio::test]
    async fn une_liste_de_collections_mongodb_refuse_un_schema() {
        let dir = TempDir::new().unwrap();
        let contexts = contexts(&dir);
        let result = tools::list_tables(
            &contexts,
            ListTablesArgs {
                connection: MONGO.into(),
                schema: Some("public".into()),
                ..ListTablesArgs::default()
            },
        )
        .await;
        assert_eq!(error_of(&result)["code"], "mongo_no_schema");
        assert!(!dir.path().join("command-ran").exists());
    }

    #[tokio::test]
    async fn une_base_par_defaut_systeme_est_refusee_avant_tout_envoi() {
        let dir = TempDir::new().unwrap();
        let contexts = contexts(&dir);
        let find = tools::find(
            &contexts,
            FindArgs {
                connection: "atlas-admin".into(),
                collection: "customers".into(),
                ..FindArgs::default()
            },
        )
        .await;
        assert_eq!(error_of(&find)["code"], "mongo_system_namespace");
        let aggregate = tools::aggregate(
            &contexts,
            AggregateArgs {
                connection: "atlas-admin".into(),
                collection: "customers".into(),
                pipeline: vec![json!({"$match": {}})],
                ..AggregateArgs::default()
            },
        )
        .await;
        assert_eq!(error_of(&aggregate)["code"], "mongo_system_namespace");
        assert!(!dir.path().join("command-ran").exists());
    }

    #[tokio::test]
    async fn sans_base_declaree_ni_demandee_la_base_est_exigee() {
        let dir = TempDir::new().unwrap();
        let contexts = contexts(&dir);
        let result = tools::find(
            &contexts,
            FindArgs {
                connection: MONGO.into(),
                collection: "customers".into(),
                ..FindArgs::default()
            },
        )
        .await;
        assert_eq!(error_of(&result)["code"], "mongo_database_required");
        assert!(!dir.path().join("command-ran").exists());
    }
}
