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
            .max_by(|(pattern, decision), (other, other_decision)| {
                pattern
                    .precedence(other)
                    .then_with(|| is_personal(decision).cmp(&is_personal(other_decision)))
                    .then_with(|| other.path.cmp(&pattern.path))
            })
            .map(|(_, decision)| decision)
    }

    pub fn has_personal_below(&self, source: &FieldSource) -> bool {
        self.entries
            .iter()
            .any(|(pattern, decision)| is_personal(decision) && pattern.lies_below(source))
    }
}

fn is_personal(decision: &PolicyDecision) -> bool {
    matches!(decision, PolicyDecision::Personal(_))
}
