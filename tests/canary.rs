mod common;

use common::{
    CONNECTION, DescribeTableArgs, ExplainArgs, Harness, ListTablesArgs, QueryArgs,
    SearchSchemaArgs, assert_no_canary, body, describe_table, explain, is_error, list_connections,
    list_tables, query, search_schema, text_of,
};
use rmcp::model::CallToolResult;
use serde_json::Value;

struct Transcript {
    responses: Vec<String>,
}

impl Transcript {
    fn record(&mut self, result: &CallToolResult) -> Value {
        let text = text_of(result);
        assert_no_canary(&text);
        self.responses.push(text.clone());
        serde_json::from_str(&text).unwrap_or(Value::Null)
    }
}

fn sql(text: &str) -> QueryArgs {
    QueryArgs {
        connection: CONNECTION.to_string(),
        sql: text.to_string(),
        ..QueryArgs::default()
    }
}

mod e2e {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn la_sortie_d_une_commande_de_mot_de_passe_n_apparait_jamais_dans_une_reponse_reussie() {
        let url = common::database_url();
        let password = url
            .strip_prefix("postgres://")
            .and_then(|rest| rest.split_once('@'))
            .and_then(|(credentials, _)| credentials.split_once(':'))
            .map(|(_, password)| password.to_string())
            .expect("user:password");
        let harness = Harness::with_password_command("/bin/echo", &[&password]);
        let state = &harness.state;

        let connections = list_connections(state).await;
        assert_eq!(body(&connections)["connections"][0]["status"], "available");

        let answer = query(state, sql("SELECT 1 AS one")).await;
        assert!(!is_error(&answer), "{}", text_of(&answer));
        assert_eq!(body(&answer)["rows"][0][0], 1);
        assert!(!text_of(&answer).contains(&password));
    }

    #[tokio::test]
    #[ignore]
    async fn une_enquete_d_agent_ne_recoit_jamais_une_valeur_reelle_semee() {
        let harness = Harness::new();
        let state = &harness.state;
        let mut transcript = Transcript {
            responses: Vec::new(),
        };

        let connections = transcript.record(&list_connections(state).await);
        assert_eq!(connections["connections"][0]["status"], "available");

        transcript.record(
            &search_schema(
                state,
                SearchSchemaArgs {
                    connection: CONNECTION.into(),
                    database: None,
                    pattern: "mail".into(),
                },
            )
            .await,
        );
        transcript.record(
            &list_tables(
                state,
                ListTablesArgs {
                    connection: CONNECTION.into(),
                    database: None,
                    schema: None,
                    name_like: None,
                    limit: None,
                    offset: None,
                },
            )
            .await,
        );
        for table in ["customers", "orders", "notes"] {
            transcript.record(
                &describe_table(
                    state,
                    DescribeTableArgs {
                        connection: CONNECTION.into(),
                        database: None,
                        table: table.into(),
                    },
                )
                .await,
            );
        }

        let customers =
            transcript.record(&query(state, sql("SELECT * FROM customers ORDER BY id")).await);
        let email = customers["rows"][0][3].as_str().unwrap().to_string();
        let full_name = transcript
            .record(&query(state, sql("SELECT titulaire FROM orders WHERE id = 10")).await)["rows"]
            [0][0]
            .as_str()
            .unwrap()
            .to_string();

        let orders = transcript.record(
            &query(
                state,
                sql(&format!(
                    "SELECT o.id, o.status, o.amount FROM orders o JOIN customers c ON c.id = o.customer_id WHERE c.email = '{email}' ORDER BY o.id"
                )),
            )
            .await,
        );
        assert_eq!(orders["translated_literals"], 1);
        assert_eq!(orders["row_count"], 2);

        let by_holder = transcript.record(
            &query(
                state,
                sql(&format!(
                    "SELECT count(*) FROM orders WHERE titulaire = '{full_name}'"
                )),
            )
            .await,
        );
        assert_eq!(by_holder["translated_literals"], 1, "{by_holder}");
        assert_eq!(by_holder["rows"][0][0], 1);

        transcript.record(
            &query(
                state,
                sql("SELECT c.ville, o.status, count(*), sum(o.amount) FROM orders o JOIN customers c ON c.id = o.customer_id GROUP BY 1, 2 ORDER BY 1, 2"),
            )
            .await,
        );
        transcript.record(&query(state, sql("SELECT body, profile, emails_secondaires FROM notes n JOIN customers c ON c.id = n.customer_id")).await);

        let failure = query(
            state,
            sql(&format!(
                "SELECT id FROM customers WHERE id = '{email}'::int"
            )),
        )
        .await;
        assert!(is_error(&failure));
        transcript.record(&failure);

        let plan = explain(
            state,
            ExplainArgs {
                connection: CONNECTION.into(),
                database: None,
                sql: format!("SELECT * FROM customers WHERE email = '{email}'"),
            },
        )
        .await;
        assert_eq!(body(&plan)["translated_literals"], 1);
        transcript.record(&plan);

        assert!(transcript.responses.len() >= 14);
    }
}
