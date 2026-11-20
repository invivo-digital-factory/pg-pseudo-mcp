use std::sync::Arc;

use serde_json::Value;

use crate::contexts::anonymization::domain::services::anonymizer::Anonymizer;
use crate::contexts::anonymization::domain::services::pseudonymizer::Pseudonymizer;

#[derive(Clone)]
pub struct AnonymizePlan {
    pseudonymizer: Arc<Pseudonymizer>,
}

impl AnonymizePlan {
    pub fn new(pseudonymizer: Arc<Pseudonymizer>) -> Self {
        Self { pseudonymizer }
    }

    pub fn execute(&self, plan: Value, injected: &[(String, String)]) -> Value {
        scrub(plan, &Anonymizer::new(&self.pseudonymizer, injected))
    }
}

fn scrub(value: Value, anonymizer: &Anonymizer) -> Value {
    match value {
        Value::String(text) => Value::String(anonymizer.free_text(&text)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| scrub(item, anonymizer))
                .collect(),
        ),
        Value::Object(entries) => Value::Object(
            entries
                .into_iter()
                .map(|(key, item)| (anonymizer.free_text(&key), scrub(item, anonymizer)))
                .collect(),
        ),
        other => other,
    }
}
