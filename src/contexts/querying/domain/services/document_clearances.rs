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
    let computed_root = command == DocumentCommand::Aggregate && lineage.root_is_computed();
    let root = computed_root.then(|| {
        (
            Vec::new(),
            DocumentClearance::Derived {
                proven_clear: false,
            },
        )
    });
    root.into_iter()
        .chain(paths.iter().filter_map(|path| {
            if path.iter().any(|segment| segment.contains('.')) {
                return Some((
                    path.clone(),
                    DocumentClearance::Derived {
                        proven_clear: false,
                    },
                ));
            }
            let traced = lineage.own_of_segments(path);
            let judged = match command {
                DocumentCommand::Find => find_clearance(path, &traced, policy, source),
                DocumentCommand::Aggregate => output_clearance(path, &traced, policy),
            };
            judged.map(|clearance| (path.clone(), clearance))
        }))
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

#[cfg(test)]
mod unit {
    use serde_json::json;

    use super::*;
    use crate::contexts::querying::domain::services::mongo_guard::{
        inspect_aggregate, inspect_find,
    };
    use crate::contexts::querying::domain::services::mongo_lineage::document_lineage;
    use crate::contexts::querying::domain::types::{
        AggregateRequest, FindRequest, MongoFieldPattern, QueryLimits, Translations,
    };

    fn segments(path: &[&str]) -> Vec<String> {
        path.iter().map(|segment| segment.to_string()).collect()
    }

    fn judged(
        lineage: &DocumentLineage,
        command: DocumentCommand,
        paths: &[&[&str]],
    ) -> Vec<(Vec<String>, DocumentClearance)> {
        let observed = paths.iter().map(|path| segments(path)).collect();
        document_clearances(
            &observed,
            lineage,
            &ColumnPolicy::default(),
            ("shop", "customers"),
            command,
        )
    }

    #[test]
    fn un_chemin_dont_une_cle_contient_un_point_est_toujours_souille() {
        let guarded = inspect_aggregate(&AggregateRequest {
            database: Some("shop".into()),
            collection: "customers".into(),
            pipeline: vec![json!({"$replaceWith": {"$arrayToObject": [[{"k": "$contact.email", "v": "$telephone"}]]}})],
            ..AggregateRequest::default()
        })
        .expect("allowed")
        .finish(&Translations::default(), &QueryLimits::default());
        let lineage = document_lineage(&guarded, "shop");
        let projected = inspect_aggregate(&AggregateRequest {
            database: Some("shop".into()),
            collection: "customers".into(),
            pipeline: vec![
                json!({"$project": {"status": 1, "who": {"$concat": ["$firstName", "$lastName"]}}}),
            ],
            ..AggregateRequest::default()
        })
        .expect("allowed")
        .finish(&Translations::default(), &QueryLimits::default());
        assert!(
            judged(
                &document_lineage(&projected, "shop"),
                DocumentCommand::Aggregate,
                &[]
            )
            .iter()
            .all(|(path, _)| !path.is_empty())
        );
        let dotted: &[&str] = &["jean.dupont@example.org"];
        let below: &[&str] = &["preferences", "jean.dupont@example.org", "newsletter"];
        let clearances = judged(&lineage, DocumentCommand::Aggregate, &[dotted, below]);
        assert!(
            clearances.contains(&(
                Vec::new(),
                DocumentClearance::Derived {
                    proven_clear: false
                }
            )),
            "{clearances:?}"
        );
        for path in [dotted, below] {
            assert!(
                clearances.contains(&(
                    segments(path),
                    DocumentClearance::Derived {
                        proven_clear: false
                    }
                )),
                "{path:?} in {clearances:?}"
            );
        }

        let found = inspect_find(&FindRequest {
            database: Some("shop".into()),
            collection: "customers".into(),
            ..FindRequest::default()
        })
        .expect("allowed")
        .finish(&Translations::default(), &QueryLimits::default());
        let clearances = judged(
            &document_lineage(&found, "shop"),
            DocumentCommand::Find,
            &[below],
        );
        assert_eq!(
            clearances,
            vec![(
                segments(below),
                DocumentClearance::Derived {
                    proven_clear: false
                }
            )]
        );
    }

    fn clear_on(paths: &[&str]) -> ColumnPolicy {
        let mut policy = ColumnPolicy::default();
        for path in paths {
            policy.insert_field(
                MongoFieldPattern {
                    database: Some("shop".into()),
                    collection: Some("customers".into()),
                    path: path.to_string(),
                },
                PolicyDecision::Clear,
            );
        }
        policy
    }

    fn found(projection: Option<serde_json::Value>) -> DocumentLineage {
        let guarded = inspect_find(&FindRequest {
            database: Some("shop".into()),
            collection: "customers".into(),
            projection,
            ..FindRequest::default()
        })
        .expect("allowed")
        .finish(&Translations::default(), &QueryLimits::default());
        document_lineage(&guarded, "shop")
    }

    fn aggregated(pipeline: Vec<serde_json::Value>) -> DocumentLineage {
        let guarded = inspect_aggregate(&AggregateRequest {
            database: Some("shop".into()),
            collection: "customers".into(),
            pipeline,
            ..AggregateRequest::default()
        })
        .expect("allowed")
        .finish(&Translations::default(), &QueryLimits::default());
        document_lineage(&guarded, "shop")
    }

    fn judged_under(
        policy: &ColumnPolicy,
        lineage: &DocumentLineage,
        command: DocumentCommand,
        paths: &[&str],
    ) -> Vec<(Vec<String>, DocumentClearance)> {
        let observed = paths
            .iter()
            .map(|path| path.split('.').map(str::to_string).collect())
            .collect();
        document_clearances(&observed, lineage, policy, ("shop", "customers"), command)
    }

    #[test]
    fn un_document_que_la_requete_ne_fait_que_traverser_est_juge_comme_sans_projection() {
        let policy = clear_on(&["attributes.level.label", "attributes.supplier.label"]);
        let paths = [
            "attributes",
            "attributes.level",
            "attributes.level.label",
            "attributes.supplier",
            "attributes.supplier.label",
            "attributes.supplier.contact",
        ];
        let inclusion = json!({
            "attributes.level.label": 1,
            "attributes.supplier.label": 1,
            "attributes.supplier.contact": 1
        });
        for (command, traversed, whole) in [
            (
                DocumentCommand::Find,
                found(Some(inclusion.clone())),
                found(None),
            ),
            (
                DocumentCommand::Find,
                found(Some(json!({"attributes.secret": 0}))),
                found(None),
            ),
            (
                DocumentCommand::Aggregate,
                aggregated(vec![json!({"$project": inclusion.clone()})]),
                aggregated(Vec::new()),
            ),
            (
                DocumentCommand::Aggregate,
                aggregated(vec![
                    json!({"$set": {"attributes.main_category": "$category"}}),
                ]),
                aggregated(Vec::new()),
            ),
        ] {
            let expected = judged_under(&policy, &whole, command, &paths);
            assert_eq!(
                expected,
                vec![
                    (
                        segments(&["attributes", "level", "label"]),
                        DocumentClearance::Clear
                    ),
                    (
                        segments(&["attributes", "supplier", "label"]),
                        DocumentClearance::Clear
                    ),
                ]
            );
            assert_eq!(
                judged_under(&policy, &traversed, command, &paths),
                expected,
                "{command:?}"
            );
        }

        let set = aggregated(vec![
            json!({"$set": {"attributes.main_category": "$category"}}),
        ]);
        assert_eq!(
            judged_under(
                &policy,
                &set,
                DocumentCommand::Aggregate,
                &["attributes.main_category"]
            ),
            vec![(
                segments(&["attributes", "main_category"]),
                DocumentClearance::NamedAs("category".into())
            )]
        );
    }

    #[test]
    fn un_document_dont_le_contenu_est_calcule_reste_derive_et_souille() {
        let policy = clear_on(&["contact", "address", "contact.email", "name"]);
        let lineage = aggregated(vec![
            json!({"$project": {"contact": {"email": 1, "label": "$name"}, "card": {"$mergeObjects": ["$contact", "$address"]}}}),
            json!({"$set": {"card.extra": 1}}),
        ]);
        let tainted = DocumentClearance::Derived {
            proven_clear: false,
        };
        assert_eq!(
            judged_under(
                &policy,
                &lineage,
                DocumentCommand::Aggregate,
                &["card", "contact"]
            ),
            vec![
                (segments(&["card"]), tainted),
                (
                    segments(&["contact"]),
                    DocumentClearance::Derived { proven_clear: true }
                ),
            ]
        );
    }
}
