use std::collections::HashMap;

use super::{ColumnPattern, ColumnSource, PolicyDecision};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ColumnPolicy {
    entries: HashMap<ColumnPattern, PolicyDecision>,
}

impl ColumnPolicy {
    pub fn insert(&mut self, pattern: ColumnPattern, decision: PolicyDecision) {
        self.entries.insert(pattern, decision);
    }

    pub fn extend(&mut self, later: ColumnPolicy) {
        self.entries.extend(later.entries);
    }

    pub fn decide(&self, source: &ColumnSource) -> Option<&PolicyDecision> {
        self.entries
            .iter()
            .filter(|(pattern, _)| pattern.matches(source))
            .max_by_key(|(pattern, _)| pattern.specificity())
            .map(|(_, decision)| decision)
    }

    pub fn has_clear(&self) -> bool {
        self.entries
            .values()
            .any(|decision| *decision == PolicyDecision::Clear)
    }

    pub fn is_personal_name(&self, column: &str) -> bool {
        let column = column.to_lowercase();
        self.entries.iter().any(|(pattern, decision)| {
            pattern.column == column && matches!(decision, PolicyDecision::Personal(_))
        })
    }
}
