use std::collections::{BTreeMap, HashMap, HashSet};

use super::classify::classify_named;
use super::dictionary::{dictionary_entry, tokenize};
use super::tracker::Tracker;
use crate::contexts::anonymization::domain::types::{
    ColumnClass, ColumnFlag, FieldClearance, FieldPath, Kind, Reason,
};

const MAX_FLAGGED_PATHS: usize = 200;

const MIN_SINGULAR_CHARS: usize = 2;

pub struct FieldClearances(HashMap<Vec<String>, FieldClearance>);

impl FieldClearances {
    pub fn new(clearances: &[(FieldPath, FieldClearance)]) -> Self {
        Self(
            clearances
                .iter()
                .map(|(path, clearance)| (folded(path), clearance.clone()))
                .collect(),
        )
    }

    fn get(&self, path: &FieldPath) -> Option<FieldClearance> {
        self.0.get(&folded(path)).cloned()
    }
}

fn folded(path: &FieldPath) -> Vec<String> {
    path.segments()
        .iter()
        .map(|segment| segment.to_lowercase())
        .collect()
}

fn is_tainted(clearance: Option<&FieldClearance>) -> bool {
    matches!(
        clearance,
        Some(FieldClearance::Derived {
            proven_clear: false
        })
    )
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
        let inherited = self.clearance.clone().filter(|clearance| {
            !matches!(
                clearance,
                FieldClearance::Clear | FieldClearance::NamedAs(_)
            )
        });
        let clearance = match clearances.get(&source) {
            Some(FieldClearance::Personal(kind)) => Some(FieldClearance::Personal(kind)),
            Some(_) if is_tainted(self.clearance.as_ref()) => inherited,
            Some(FieldClearance::NamedAs(name)) => {
                inherited.or(Some(FieldClearance::NamedAs(name)))
            }
            Some(own) => Some(own),
            None => inherited,
        };
        let name_kind = match &clearance {
            Some(FieldClearance::Clear) => None,
            Some(FieldClearance::NamedAs(name)) => key_entry(name)
                .flatten()
                .or_else(|| key_entry(key).unwrap_or(self.name_kind)),
            _ => key_entry(key).unwrap_or(self.name_kind),
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
        match &self.clearance {
            None | Some(FieldClearance::NamedAs(_)) => {
                (classify_named(self.name_kind, type_name, false), false)
            }
            Some(FieldClearance::Clear) => (
                ColumnClass {
                    structural_only: true,
                    ..classify_named(None, type_name, false)
                },
                false,
            ),
            Some(FieldClearance::Personal(kind)) => (
                ColumnClass {
                    forced_kind: Some(*kind),
                    clear_by_type: false,
                    derived: false,
                    structural_only: false,
                    forced_by_policy: true,
                },
                false,
            ),
            Some(FieldClearance::Derived { proven_clear }) => {
                let proven_clear = *proven_clear;
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

fn key_entry(key: &str) -> Option<Option<Kind>> {
    dictionary_entry(key).or_else(|| {
        let singular: Vec<String> = tokenize(key).into_iter().map(singular).collect();
        dictionary_entry(&singular.join("_"))
    })
}

fn singular(token: String) -> String {
    if let Some(stem) = token.strip_suffix("aux")
        && stem.chars().count() >= MIN_SINGULAR_CHARS
    {
        return format!("{stem}al");
    }
    match token.strip_suffix('s') {
        Some(stem) if stem.chars().count() >= MIN_SINGULAR_CHARS => stem.to_string(),
        _ => token,
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
        let path = path.dotted();
        if self.kept.contains_key(&path) || self.omitted.contains(&path) {
            return;
        }
        if self.kept.len() >= MAX_FLAGGED_PATHS {
            self.omitted.insert(path);
            return;
        }
        let why = if derived { Reason::Derived } else { reason };
        self.kept.insert(
            path,
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
