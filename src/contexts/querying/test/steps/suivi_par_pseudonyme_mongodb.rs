use cucumber::gherkin::Step;
use cucumber::{given, then};
use scarif_database_mcp::contexts::querying::domain::types::{GuardedMongo, MongoCommand};
use serde_json::Value;

use crate::querying_world::QueryingWorld;

fn accepted(world: &QueryingWorld) -> GuardedMongo {
    world
        .mongo_answer()
        .outcome
        .clone()
        .unwrap_or_else(|refusal| panic!("refused: {refusal}"))
}

fn sent_filter(world: &QueryingWorld) -> Value {
    match accepted(world).command {
        MongoCommand::Find(command) => command.filter,
        MongoCommand::Aggregate(command) => Value::Array(command.pipeline),
    }
}

#[given(regex = r#"^le pseudonyme "([^"]+)" désigne la valeur réelle "([^"]+)"$"#)]
async fn pseudonym_maps_to(world: &mut QueryingWorld, pseudonym: String, real: String) {
    world.mongo_pseudonyms.push((pseudonym, real));
}

#[then(regex = r"^(\d+) valeur a été traduite$")]
async fn values_translated(world: &mut QueryingWorld, count: usize) {
    assert_eq!(accepted(world).translated_literals, count);
}

#[then(regex = r"^le filtre envoyé ne contient plus le pseudonyme$")]
async fn filter_has_no_pseudonym(world: &mut QueryingWorld) {
    let text = sent_filter(world).to_string();
    for (pseudonym, real) in &world.mongo_pseudonyms {
        assert!(!text.contains(pseudonym.as_str()), "{text}");
        assert!(text.contains(real.as_str()), "{text}");
    }
}

#[then(regex = r#"^la demande porte l'avertissement "([^"]+)"$"#)]
async fn request_carries_warning(world: &mut QueryingWorld, code: String) {
    let warnings = accepted(world).warnings;
    assert!(
        warnings.iter().any(|warning| warning.code == code),
        "{warnings:?}"
    );
}

#[then(regex = r"^le filtre envoyé vaut :$")]
async fn filter_equals(world: &mut QueryingWorld, step: &Step) {
    let expected: Value =
        serde_json::from_str(step.docstring.as_deref().expect("a docstring")).expect("JSON");
    assert_eq!(sent_filter(world), expected);
}
