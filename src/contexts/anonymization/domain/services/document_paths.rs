use std::collections::{BTreeMap, HashMap, HashSet};

use super::classify::classify_named;
use super::dictionary::dictionary_kind;
use super::tracker::Tracker;
use crate::contexts::anonymization::domain::types::{
    ColumnClass, ColumnFlag, FieldClearance, FieldPath, Kind, Reason,
};

const MAX_FLAGGED_PATHS: usize = 200;

pub struct FieldClearances(HashMap<String, FieldClearance>);

impl FieldClearances {
    pub fn new(clearances: &[(FieldPath, FieldClearance)]) -> Self {
        Self(
            clearances
                .iter()
                .map(|(path, clearance)| (path.as_str().to_lowercase(), *clearance))
                .collect(),
        )
    }

    fn get(&self, path: &FieldPath) -> Option<FieldClearance> {
        self.0.get(&path.as_str().to_lowercase()).copied()
    }
}

#[derive(Default)]
pub struct Place {
    source: FieldPath,
    shown: FieldPath,
    name_kind: Option<Kind>,
    clearance: Option<FieldClearance>,
}

impl Place {
    pub fn child(&self, key: &str, shown: &str, clearances: &FieldClearances) -> Self {
        let source = self.source.child(key);
        let inherited = self
            .clearance
            .filter(|clearance| *clearance != FieldClearance::Clear);
        let clearance = clearances.get(&source).or(inherited);
        let name_kind = match clearance {
            Some(FieldClearance::Clear) => None,
            _ => dictionary_kind(key),
        };
        Self {
            source,
            shown: self.shown.child(shown),
            name_kind,
            clearance,
        }
    }

    pub fn shown(&self) -> &FieldPath {
        &self.shown
    }

    pub fn class(&self, type_name: &str) -> (ColumnClass, bool) {
        match self.clearance {
            None => (classify_named(self.name_kind, type_name, false), false),
            Some(FieldClearance::Clear) => (
                ColumnClass {
                    structural_only: true,
                    ..classify_named(None, type_name, false)
                },
                false,
            ),
            Some(FieldClearance::Personal(kind)) => (
                ColumnClass {
                    forced_kind: Some(kind),
                    clear_by_type: false,
                    derived: false,
                    structural_only: false,
                    forced_by_policy: true,
                },
                false,
            ),
            Some(FieldClearance::Derived { proven_clear }) => {
                let class = classify_named(self.name_kind, type_name, true);
                if proven_clear && class.forced_kind.is_none() {
                    (
                        ColumnClass {
                            forced_kind: None,
                            clear_by_type: false,
                            derived: false,
                            structural_only: true,
                            forced_by_policy: false,
                        },
                        false,
                    )
                } else {
                    (class, !proven_clear)
                }
            }
        }
    }
}

#[derive(Default)]
pub struct PathFlags {
    kept: BTreeMap<String, ColumnFlag>,
    omitted: HashSet<String>,
}

impl PathFlags {
    pub fn record(&mut self, path: &FieldPath, tracker: &Tracker, derived: bool) {
        let Some((kind, reason)) = tracker.first else {
            return;
        };
        let path = path.as_str();
        if self.kept.contains_key(path) || self.omitted.contains(path) {
            return;
        }
        if self.kept.len() >= MAX_FLAGGED_PATHS {
            self.omitted.insert(path.to_string());
            return;
        }
        let why = if derived { Reason::Derived } else { reason };
        self.kept.insert(
            path.to_string(),
            ColumnFlag {
                kind: kind.to_string(),
                why,
            },
        );
    }

    pub fn into_parts(self) -> (BTreeMap<String, ColumnFlag>, usize) {
        (self.kept, self.omitted.len())
    }
}
