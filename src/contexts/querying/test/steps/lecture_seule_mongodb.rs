use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use scarif_database_mcp::contexts::querying::domain::repositories::PseudonymTranslator;
use scarif_database_mcp::contexts::querying::domain::services::mongo_guard::{
    InspectedMongo, inspect_aggregate, inspect_find,
};
use scarif_database_mcp::contexts::querying::domain::types::{
    AggregateRequest, FindRequest, MongoCommand, PseudonymLookup, QueryLimits, Translations,
};
use serde_json::Value;

use crate::querying_world::{MongoAnswer, QueryingWorld};

#[derive(Default)]
struct FakePseudonymStore {
    pseudonyms: HashMap<String, String>,
    lookups: AtomicUsize,
}

impl PseudonymTranslator for FakePseudonymStore {
    fn lookup(&self, text: &str) -> PseudonymLookup {
        self.lookups.fetch_add(1, Ordering::SeqCst);
        if let Some(real) = self.pseudonyms.get(text) {
            return PseudonymLookup::Known { real: real.clone() };
        }
        if text.ends_with("@example.org") || text.ends_with("@example.net") {
            PseudonymLookup::Unknown
        } else {
            PseudonymLookup::NotAPseudonym
        }
    }
}

fn docstring_json(step: &Step) -> Value {
    let text = step.docstring.as_deref().expect("a docstring");
    serde_json::from_str(text).expect("a JSON docstring")
}

fn guard(world: &mut QueryingWorld, inspected: Result<InspectedMongo, String>) {
    let store = FakePseudonymStore {
        pseudonyms: world.mongo_pseudonyms.iter().cloned().collect(),
        lookups: AtomicUsize::new(0),
    };
    let outcome = inspected.map(|inspected| {
        let mut translations = Translations::default();
        for literal in inspected.literals() {
            translations.record(&literal, store.lookup(&literal));
        }
        inspected.finish(&translations, &QueryLimits::default())
    });
    world.mongo = Some(MongoAnswer {
        outcome,
        lookups: store.lookups.load(Ordering::SeqCst),
    });
}

fn refusal_text(world: &QueryingWorld) -> String {
    world.mongo_answer().outcome.clone().expect_err("a refusal")
}

#[given(regex = r"^l'agent connecté à une base MongoDB$")]
async fn agent_connected_to_mongodb(world: &mut QueryingWorld) {
    world.mongo_pseudonyms.clear();
    world.mongo = None;
}

#[when(regex = r"^l'agent lance l'agrégation suivante :$")]
async fn agent_runs_aggregate(world: &mut QueryingWorld, step: &Step) {
    let pipeline = docstring_json(step);
    let request = AggregateRequest {
        connection: "MONGO".into(),
        database: Some("shop".into()),
        collection: "orders".into(),
        pipeline: pipeline.as_array().expect("a pipeline array").clone(),
        ..AggregateRequest::default()
    };
    let inspected = inspect_aggregate(&request)
        .map_err(|refusal| format!("{} {} {:?}", refusal.code, refusal.message, refusal.hint));
    guard(world, inspected);
}

#[when(regex = r"^l'agent cherche avec le filtre suivant :$")]
async fn agent_runs_find(world: &mut QueryingWorld, step: &Step) {
    let request = FindRequest {
        connection: "MONGO".into(),
        database: Some("shop".into()),
        collection: "customers".into(),
        filter: Some(docstring_json(step)),
        ..FindRequest::default()
    };
    let inspected = inspect_find(&request)
        .map_err(|refusal| format!("{} {} {:?}", refusal.code, refusal.message, refusal.hint));
    guard(world, inspected);
}

#[then(regex = r"^la demande est acceptée$")]
async fn request_accepted(world: &mut QueryingWorld) {
    if let Err(refusal) = &world.mongo_answer().outcome {
        panic!("refused: {refusal}");
    }
}

#[then(regex = r#"^la demande est refusée par la règle "([^"]+)"$"#)]
async fn request_refused_by(world: &mut QueryingWorld, rule: String) {
    let text = refusal_text(world);
    assert!(text.starts_with(&format!("{rule} ")), "{text}");
}

#[then(regex = r#"^le refus nomme "([^"]+)"$"#)]
async fn refusal_names(world: &mut QueryingWorld, name: String) {
    let text = refusal_text(world);
    assert!(text.contains(&name), "{text}");
}

#[then(regex = r"^rien n'a été demandé au magasin de pseudonymes$")]
async fn store_untouched(world: &mut QueryingWorld) {
    assert_eq!(world.mongo_answer().lookups, 0);
}

#[then(regex = r"^le refus ne contient aucune valeur canari$")]
async fn refusal_has_no_canary(world: &mut QueryingWorld) {
    let text = refusal_text(world).to_lowercase();
    for fragment in ["ylwenna", "kervadec", "canary-mail"] {
        assert!(!text.contains(fragment), "{text}");
    }
}

#[then(regex = r"^le pipeline envoyé se termine par une limite de (\d+) documents$")]
async fn pipeline_ends_with_limit(world: &mut QueryingWorld, limit: u64) {
    let guarded = world.mongo_answer().outcome.clone().expect("accepted");
    let MongoCommand::Aggregate(command) = guarded.command else {
        panic!("an aggregate command");
    };
    assert_eq!(
        command.pipeline.last(),
        Some(&serde_json::json!({ "$limit": limit }))
    );
}
