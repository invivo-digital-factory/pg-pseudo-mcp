use cucumber::{then, when};
use scarif_database_mcp::composition::tools;
use scarif_database_mcp::contexts::querying::presentation::ExplainArgs;

use crate::bdd_support::CONNECTION;
use crate::querying_world::QueryingWorld;

async fn explain(world: &mut QueryingWorld, sql: String) {
    let result = tools::explain(
        world.contexts(),
        ExplainArgs {
            connection: CONNECTION.into(),
            sql,
            database: None,
        },
    )
    .await;
    world.record(&result);
}

#[when(regex = r"^l'agent demande le plan du filtre des clients sur ce pseudonyme$")]
async fn agent_explains_pseudonym_filter(world: &mut QueryingWorld) {
    let pseudonym = world.pseudonym.clone().expect("a pseudonym was read");
    explain(
        world,
        format!("SELECT * FROM customers WHERE email = '{pseudonym}'"),
    )
    .await;
}

#[when(regex = r#"^l'agent demande le plan de "(.+)"$"#)]
async fn agent_explains(world: &mut QueryingWorld, sql: String) {
    explain(world, sql).await;
}

#[then(regex = r"^le plan est rendu$")]
async fn plan_is_returned(world: &mut QueryingWorld) {
    let answer = world.answer();
    assert!(!answer.refused, "{}", answer.text);
    let plan = answer.body()["plan"].clone();
    assert!(
        plan.as_array().is_some_and(|lines| !lines.is_empty()),
        "{}",
        answer.text
    );
}
