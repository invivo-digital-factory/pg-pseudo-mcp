use std::collections::{BTreeMap, BTreeSet};

use super::column_policy::field_hint;
use super::mongo_lineage::{DocumentLineage, clearance};
use crate::contexts::querying::domain::types::{
    ColumnFlag, ColumnPolicy, DocumentClearance, DocumentNode, FieldLineage, FieldPolicyHint,
    FieldSource, LineageClearance, PolicyDecision,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentCommand {
    Find,
    Aggregate,
}

pub fn observed_paths(documents: &[DocumentNode]) -> BTreeSet<Vec<String>> {
    let mut paths = BTreeSet::new();
    for document in documents {
        collect(document, &mut Vec::new(), &mut paths);
    }
    paths
}

fn collect(node: &DocumentNode, prefix: &mut Vec<String>, paths: &mut BTreeSet<Vec<String>>) {
    match node {
        DocumentNode::Array(items) => {
            for item in items {
                collect(item, prefix, paths);
            }
        }
        DocumentNode::Object(entries) => {
            for (key, value) in entries {
                prefix.push(key.clone());
                paths.insert(prefix.clone());
                collect(value, prefix, paths);
                prefix.pop();
            }
        }
        _ => {}
    }
}

pub fn document_clearances(
    paths: &BTreeSet<Vec<String>>,
    lineage: &DocumentLineage,
    policy: &ColumnPolicy,
    source: (&str, &str),
    command: DocumentCommand,
) -> Vec<(Vec<String>, DocumentClearance)> {
    paths
        .iter()
        .filter(|path| path.iter().all(|segment| !segment.contains('.')))
        .filter_map(|path| {
            let traced = lineage.of_segments(path);
            let judged = match command {
                DocumentCommand::Find => find_clearance(path, &traced, policy, source),
                DocumentCommand::Aggregate => output_clearance(path, &traced, policy),
            };
            judged.map(|clearance| (path.clone(), clearance))
        })
        .collect()
}

fn output_clearance(
    path: &[String],
    traced: &FieldLineage,
    policy: &ColumnPolicy,
) -> Option<DocumentClearance> {
    match clearance(traced, policy) {
        LineageClearance::Source(source) => source_clearance(path, &source, policy),
        LineageClearance::Derived { proven_clear } => {
            Some(DocumentClearance::Derived { proven_clear })
        }
    }
}

fn source_clearance(
    path: &[String],
    source: &FieldSource,
    policy: &ColumnPolicy,
) -> Option<DocumentClearance> {
    match policy.decide_field(source) {
        Some(PolicyDecision::Clear) => Some(DocumentClearance::Clear),
        Some(PolicyDecision::Personal(kind)) => Some(DocumentClearance::Personal(kind.clone())),
        None => {
            let source_leaf = source.path.rsplit('.').next().unwrap_or_default();
            let output_leaf = path.last().map(String::as_str).unwrap_or_default();
            (!source_leaf.is_empty() && !source_leaf.eq_ignore_ascii_case(output_leaf))
                .then(|| DocumentClearance::NamedAs(source_leaf.to_string()))
        }
    }
}

fn find_clearance(
    path: &[String],
    traced: &FieldLineage,
    policy: &ColumnPolicy,
    (database, collection): (&str, &str),
) -> Option<DocumentClearance> {
    let unprojected = FieldSource {
        database: database.to_string(),
        collection: collection.to_string(),
        path: path.join("."),
    };
    match traced {
        FieldLineage::Source(source) if *source == unprojected => {
            source_clearance(path, source, policy)
        }
        _ => {
            let projected_clear = match clearance(traced, policy) {
                LineageClearance::Source(source) => {
                    policy.decide_field(&source) == Some(&PolicyDecision::Clear)
                }
                LineageClearance::Derived { proven_clear } => proven_clear,
            };
            let stored_clear = policy.decide_field(&unprojected) == Some(&PolicyDecision::Clear)
                && !policy.has_personal_field_below(&unprojected);
            Some(DocumentClearance::Derived {
                proven_clear: projected_clear && stored_clear,
            })
        }
    }
}

pub fn document_hints(
    pseudonymized: &BTreeMap<String, ColumnFlag>,
    lineage: &DocumentLineage,
    policy: &ColumnPolicy,
) -> Vec<FieldPolicyHint> {
    let mut hints: Vec<FieldPolicyHint> = Vec::new();
    for (path, flag) in pseudonymized {
        let FieldLineage::Source(source) = lineage.of(path) else {
            continue;
        };
        let Some(hint) = field_hint(policy, &source, flag.why, &flag.kind) else {
            continue;
        };
        if !hints
            .iter()
            .any(|known| known.collection == hint.collection && known.field == hint.field)
        {
            hints.push(hint);
        }
    }
    hints
}
