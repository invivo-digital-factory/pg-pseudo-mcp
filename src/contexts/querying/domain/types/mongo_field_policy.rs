use std::collections::HashMap;

use super::{FieldSource, MongoFieldPattern, PolicyDecision};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MongoFieldPolicy {
    entries: HashMap<MongoFieldPattern, PolicyDecision>,
}

impl MongoFieldPolicy {
    pub fn insert(&mut self, pattern: MongoFieldPattern, decision: PolicyDecision) {
        self.entries.insert(pattern, decision);
    }

    pub fn extend(&mut self, later: MongoFieldPolicy) {
        self.entries.extend(later.entries);
    }

    pub fn decide(&self, source: &FieldSource) -> Option<&PolicyDecision> {
        self.entries
            .iter()
            .filter(|(pattern, _)| pattern.matches(source))
            .max_by_key(|(pattern, _)| pattern.specificity())
            .map(|(_, decision)| decision)
    }

    pub fn has_personal_below(&self, source: &FieldSource) -> bool {
        let prefix = format!("{}.", source.path.to_lowercase());
        self.entries.iter().any(|(pattern, decision)| {
            matches!(decision, PolicyDecision::Personal(_))
                && pattern.path.starts_with(&prefix)
                && pattern.covers_collection(source)
        })
    }
}
