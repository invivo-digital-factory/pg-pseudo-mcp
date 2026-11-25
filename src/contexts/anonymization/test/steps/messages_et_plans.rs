use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use scarif_database_mcp::contexts::anonymization::domain::types::DatabaseMessage;
use serde_json::Value;

use crate::anonymization_world::{AnonymizationWorld, listed, quoted};

const EXECUTION_CONTEXT: &str = "SQL function \"f\" statement 1";

fn answer(world: &mut AnonymizationWorld, sqlstate: &str, message: &str, detail: Option<String>) {
    let message = DatabaseMessage {
        severity: "ERROR".to_string(),
        sqlstate: Some(sqlstate.to_string()),
        message: quoted(message),
        detail: detail.map(|detail| world.expand(&quoted(&detail))),
        hint: None,
        where_: Some(EXECUTION_CONTEXT.to_string()),
    };
    world.message = Some(
        world
            .use_cases()
            .anonymize_message
            .execute(&message, &world.injected),
    );
}

fn rendered_message(world: &AnonymizationWorld) -> String {
    world
        .message
        .as_ref()
        .map(|message| message.message.clone())
        .or_else(|| world.pseudonyms.last().cloned())
        .expect("a message was rendered")
}

fn rendered_detail(world: &AnonymizationWorld) -> String {
    world
        .message
        .as_ref()
        .and_then(|message| message.detail.clone())
        .expect("a detail was rendered")
}

fn rendered_plan(world: &AnonymizationWorld) -> &Value {
    world.plan.as_ref().expect("a plan was rendered")
}

#[given(regex = r#"^les noms connus du serveur "([^"]+)"$"#)]
async fn server_known_names(world: &mut AnonymizationWorld, names: String) {
    world.known_names = listed(&names);
}

#[when(regex = r#"^la base répond l'erreur "(\w+)" "((?:[^"\\]|\\.)*)"$"#)]
async fn database_answers(world: &mut AnonymizationWorld, sqlstate: String, message: String) {
    answer(world, &sqlstate, &message, None);
}

#[when(
    regex = r#"^la base répond l'erreur "(\w+)" "((?:[^"\\]|\\.)*)" avec le détail "((?:[^"\\]|\\.)*)"$"#
)]
async fn database_answers_with_detail(
    world: &mut AnonymizationWorld,
    sqlstate: String,
    message: String,
    detail: String,
) {
    answer(world, &sqlstate, &message, Some(detail));
}

#[when(regex = r#"^le serveur MongoDB répond "((?:[^"\\]|\\.)*)"$"#)]
async fn mongodb_answers(world: &mut AnonymizationWorld, message: String) {
    let names: Vec<&str> = world.known_names.iter().map(String::as_str).collect();
    let rendered = world.use_cases().anonymize_documents.server_message(
        &quoted(&message),
        &world.injected,
        &names,
    );
    world.message = None;
    world.pseudonyms.push(rendered);
}

#[when(regex = r"^l'agent lit le plan suivant :$")]
async fn agent_reads_plan(world: &mut AnonymizationWorld, step: &Step) {
    let text = step.docstring.as_deref().expect("a docstring");
    let plan: Value = serde_json::from_str(text).expect("a JSON plan");
    world.plan = Some(
        world
            .use_cases()
            .anonymize_plan
            .execute(plan, &world.injected),
    );
}

#[then(regex = r#"^le message rendu est "((?:[^"\\]|\\.)*)"$"#)]
async fn message_is(world: &mut AnonymizationWorld, expected: String) {
    assert_eq!(rendered_message(world), quoted(&expected));
}

#[then(regex = r#"^le message rendu ne contient plus "([^"]+)"$"#)]
async fn message_lacks(world: &mut AnonymizationWorld, fragment: String) {
    let message = rendered_message(world);
    assert!(
        !message.contains(&fragment),
        "{fragment} leaked in {message}"
    );
}

#[then(regex = r#"^le message rendu contient "((?:[^"\\]|\\.)*)"$"#)]
async fn message_contains(world: &mut AnonymizationWorld, fragment: String) {
    let message = rendered_message(world);
    let fragment = quoted(&fragment);
    assert!(
        message.contains(&fragment),
        "{fragment} missing in {message}"
    );
}

#[then(regex = r#"^le message rendu commence par "((?:[^"\\]|\\.)*)"$"#)]
async fn message_starts_with(world: &mut AnonymizationWorld, prefix: String) {
    let message = rendered_message(world);
    let prefix = quoted(&prefix);
    assert!(message.starts_with(&prefix), "{message}");
}

#[then(regex = r"^le message rendu n'a pas de contexte d'exécution$")]
async fn message_has_no_context(world: &mut AnonymizationWorld) {
    let message = world.message.as_ref().expect("a database message");
    assert_eq!(message.where_, None);
}

#[then(regex = r#"^le détail rendu est "((?:[^"\\]|\\.)*)"$"#)]
async fn detail_is(world: &mut AnonymizationWorld, expected: String) {
    assert_eq!(rendered_detail(world), world.expand(&quoted(&expected)));
}

#[then(regex = r#"^le détail rendu ne contient plus "([^"]+)"$"#)]
async fn detail_lacks(world: &mut AnonymizationWorld, fragment: String) {
    let detail = rendered_detail(world);
    assert!(!detail.contains(&fragment), "{fragment} leaked in {detail}");
}

#[then(regex = r#"^le détail rendu commence par "((?:[^"\\]|\\.)*)"$"#)]
async fn detail_starts_with(world: &mut AnonymizationWorld, prefix: String) {
    let detail = rendered_detail(world);
    assert!(detail.starts_with(&quoted(&prefix)), "{detail}");
}

#[then(regex = r#"^le détail rendu se termine par "((?:[^"\\]|\\.)*)"$"#)]
async fn detail_ends_with(world: &mut AnonymizationWorld, suffix: String) {
    let detail = rendered_detail(world);
    assert!(detail.ends_with(&quoted(&suffix)), "{detail}");
}

#[then(regex = r#"^le plan rendu ne contient plus "([^"]+)"$"#)]
async fn plan_lacks(world: &mut AnonymizationWorld, fragment: String) {
    let plan = rendered_plan(world).to_string();
    assert!(!plan.contains(&fragment), "{fragment} leaked in {plan}");
}

#[then(regex = r#"^le plan rendu garde "([^"]+)" à "([^"]+)"$"#)]
async fn plan_keeps(world: &mut AnonymizationWorld, key: String, value: String) {
    let plan = rendered_plan(world);
    assert_eq!(plan["Plan"][&key], Value::String(value), "{plan}");
}
