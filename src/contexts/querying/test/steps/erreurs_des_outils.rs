use cucumber::{given, then, when};
use scarif_database_mcp::composition::tools;
use scarif_database_mcp::contexts::querying::presentation::{
    AggregateArgs, DescribeTableArgs, ExplainArgs, FindArgs, ListDatabasesArgs, ListTablesArgs,
    QueryArgs, SearchSchemaArgs,
};
use serde_json::Value;

use crate::bdd_support::CONNECTION;
use crate::querying_world::{MONGO_CONNECTION, QueryingWorld};

fn error(world: &QueryingWorld) -> Value {
    let answer = world.answer();
    assert!(answer.refused, "{}", answer.text);
    answer.body()["error"].clone()
}

#[given(regex = r"^une connexion PostgreSQL dont le magasin de pseudonymes est inutilisable$")]
async fn store_unavailable(world: &mut QueryingWorld) {
    world.connect_without_pseudonym_store();
}

#[when(regex = r#"^l'agent exécute "(.+)" sur la connexion "([^"]+)"$"#)]
async fn agent_queries_connection(world: &mut QueryingWorld, sql: String, connection: String) {
    let result = tools::query(
        world.contexts(),
        QueryArgs {
            connection,
            sql,
            ..QueryArgs::default()
        },
    )
    .await;
    world.record(&result);
}

#[when(
    regex = r"^l'agent appelle (query|explain|find|aggregate|list_tables|describe_table|search_schema|list_databases) sur la connexion (PostgreSQL|MongoDB)$"
)]
async fn agent_calls_tool(world: &mut QueryingWorld, tool: String, engine: String) {
    let connection = if engine == "MongoDB" {
        MONGO_CONNECTION
    } else {
        CONNECTION
    }
    .to_string();
    let contexts = world.contexts();
    let result = match tool.as_str() {
        "query" => {
            tools::query(
                contexts,
                QueryArgs {
                    connection,
                    sql: "SELECT 1".into(),
                    ..QueryArgs::default()
                },
            )
            .await
        }
        "explain" => {
            tools::explain(
                contexts,
                ExplainArgs {
                    connection,
                    sql: "SELECT 1".into(),
                    database: None,
                },
            )
            .await
        }
        "find" => {
            tools::find(
                contexts,
                FindArgs {
                    connection,
                    collection: "customers".into(),
                    ..FindArgs::default()
                },
            )
            .await
        }
        "aggregate" => {
            tools::aggregate(
                contexts,
                AggregateArgs {
                    connection,
                    collection: "customers".into(),
                    ..AggregateArgs::default()
                },
            )
            .await
        }
        "list_tables" => {
            tools::list_tables(
                contexts,
                ListTablesArgs {
                    connection,
                    ..ListTablesArgs::default()
                },
            )
            .await
        }
        "describe_table" => {
            tools::describe_table(
                contexts,
                DescribeTableArgs {
                    connection,
                    table: "customers".into(),
                    database: None,
                },
            )
            .await
        }
        "search_schema" => {
            tools::search_schema(
                contexts,
                SearchSchemaArgs {
                    connection,
                    pattern: "mail".into(),
                    database: None,
                },
            )
            .await
        }
        _ => tools::list_databases(contexts, ListDatabasesArgs { connection }).await,
    };
    world.record(&result);
}

#[when(regex = r"^l'agent convertit ce pseudonyme en nombre$")]
async fn agent_casts_pseudonym(world: &mut QueryingWorld) {
    let pseudonym = world.pseudonym.clone().expect("a pseudonym was read");
    world
        .ask(&format!(
            "SELECT id FROM customers WHERE id = '{pseudonym}'::int"
        ))
        .await;
}

#[then(regex = r#"^l'erreur est de catégorie "([^"]+)"$"#)]
async fn error_category(world: &mut QueryingWorld, category: String) {
    let error = error(world);
    assert_eq!(error["category"], category.as_str(), "{error}");
}

#[then(regex = r#"^l'indice de l'erreur contient "([^"]+)"$"#)]
async fn error_hint_contains(world: &mut QueryingWorld, text: String) {
    let error = error(world);
    let hint = error["hint"].as_str().unwrap_or_default();
    assert!(hint.contains(&text), "{error}");
}

#[then(regex = r"^l'erreur (peut|ne peut pas) être retentée$")]
async fn error_retryable(world: &mut QueryingWorld, verdict: String) {
    let error = error(world);
    assert_eq!(error["retryable"], verdict == "peut", "{error}");
}

#[then(regex = r#"^l'erreur porte le code SQLSTATE "([^"]+)"$"#)]
async fn error_sqlstate(world: &mut QueryingWorld, sqlstate: String) {
    let error = error(world);
    assert_eq!(error["sqlstate"], sqlstate.as_str(), "{error}");
}

#[then(regex = r"^l'erreur porte un code_name MongoDB et aucun SQLSTATE$")]
async fn error_code_name(world: &mut QueryingWorld) {
    let error = error(world);
    assert!(error["code_name"].is_string(), "{error}");
    assert!(error.get("sqlstate").is_none(), "{error}");
}

#[then(regex = r"^la réponse cite le pseudonyme$")]
async fn answer_quotes_pseudonym(world: &mut QueryingWorld) {
    let pseudonym = world.pseudonym.clone().expect("a pseudonym was read");
    let text = &world.answer().text;
    assert!(text.contains(&pseudonym), "{text}");
}
