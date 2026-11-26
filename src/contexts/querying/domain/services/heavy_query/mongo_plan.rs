use serde_json::Value;

use crate::contexts::querying::domain::constants::heavy_query::{
    MONGO_CHILD_STAGES, MONGO_COLLSCAN, MONGO_LIMIT, MONGO_PASS_THROUGH,
};
use crate::contexts::querying::domain::types::CollectionScan;

pub fn collection_scans(explain: &Value) -> Vec<CollectionScan> {
    let mut scans = Vec::new();
    visit(explain, None, &mut scans);
    scans
}

fn visit(value: &Value, namespace: Option<&str>, scans: &mut Vec<CollectionScan>) {
    match value {
        Value::Object(fields) => {
            let namespace = fields
                .get("namespace")
                .and_then(Value::as_str)
                .or(namespace);
            for (key, child) in fields {
                match key.as_str() {
                    "winningPlan" => {
                        let plan = child.get("queryPlan").unwrap_or(child);
                        if let Some(collection) = namespace.and_then(collection_of) {
                            stage(plan, collection, false, scans);
                        }
                        visit_shards(child, namespace, scans);
                    }
                    "rejectedPlans" | "command" => {}
                    _ => visit(child, namespace, scans),
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                visit(item, namespace, scans);
            }
        }
        _ => {}
    }
}

fn visit_shards(plan: &Value, namespace: Option<&str>, scans: &mut Vec<CollectionScan>) {
    if let Some(shards) = plan.get("shards") {
        visit(shards, namespace, scans);
    }
}

fn stage(node: &Value, collection: &str, limited: bool, scans: &mut Vec<CollectionScan>) {
    let name = node.get("stage").and_then(Value::as_str).unwrap_or("");
    if name == MONGO_COLLSCAN {
        scans.push(CollectionScan {
            collection: collection.to_string(),
            filtered: node
                .get("filter")
                .and_then(Value::as_object)
                .is_some_and(|filter| !filter.is_empty()),
            stopped_by_limit: limited || node.get("limitAmount").is_some(),
        });
    }
    let child_limited = name == MONGO_LIMIT || (limited && MONGO_PASS_THROUGH.contains(&name));
    for key in MONGO_CHILD_STAGES {
        if let Some(child) = node.get(*key) {
            stage(child, collection, child_limited, scans);
        }
    }
    for child in node
        .get("inputStages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        stage(child, collection, false, scans);
    }
}

fn collection_of(namespace: &str) -> Option<&str> {
    namespace
        .split_once('.')
        .map(|(_, collection)| collection)
        .filter(|collection| !collection.is_empty())
}
