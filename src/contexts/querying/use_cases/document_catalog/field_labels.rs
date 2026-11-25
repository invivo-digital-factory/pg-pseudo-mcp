use crate::contexts::querying::domain::constants::catalog::DETECTED_PER_VALUE;
use crate::contexts::querying::domain::repositories::DocumentPorts;
use crate::contexts::querying::domain::services::column_policy::field_hint;
use crate::contexts::querying::domain::services::column_pseudonymization::policy_pseudonymization;
use crate::contexts::querying::domain::types::{
    ColumnPolicy, FieldEntry, FieldPolicyHint, FieldSample, FieldSource, PolicyDecision, Reason,
};

const CONTAINER_TYPES: [&str; 3] = ["object", "array", "null"];
const WITHHELD_TYPES: [&str; 4] = ["binData", "regex", "javascript", "javascriptWithScope"];

pub fn shown_path(ports: &DocumentPorts, segments: &[String]) -> String {
    segments
        .iter()
        .map(|segment| ports.anonymizer.key(segment))
        .collect::<Vec<_>>()
        .join(".")
}

fn value_type(types: &[String]) -> Option<&str> {
    types
        .iter()
        .map(String::as_str)
        .find(|name| *name == "string")
        .or_else(|| {
            types
                .iter()
                .map(String::as_str)
                .find(|name| !CONTAINER_TYPES.contains(name))
        })
}

pub fn field_entry(
    ports: &DocumentPorts,
    policy: &ColumnPolicy,
    (database, collection): (&str, &str),
    sample: FieldSample,
    hints: &mut Vec<FieldPolicyHint>,
) -> FieldEntry {
    let path = shown_path(ports, &sample.path);
    let addressable = sample.path.iter().all(|segment| !segment.contains('.'));
    let source = FieldSource {
        database: database.to_string(),
        collection: collection.to_string(),
        path: sample.path.join("."),
    };
    let decision = addressable.then(|| policy.decide_field(&source)).flatten();
    let leaf = sample.path.last().map(String::as_str).unwrap_or_default();
    let pseudonymized = match value_type(&sample.types) {
        None => None,
        Some(kind) if WITHHELD_TYPES.contains(&kind) => Some("withheld".to_string()),
        Some(type_name) => {
            let class = ports.dictionary.classify(leaf, type_name);
            let label = policy_pseudonymization(&class, decision, type_name);
            if decision.is_none() && addressable {
                let hinted = match (&class.forced_kind, &label) {
                    (Some(kind), Some(_)) => field_hint(policy, &source, Reason::Dictionary, kind),
                    (None, Some(label)) if label == DETECTED_PER_VALUE => {
                        field_hint(policy, &source, Reason::Default, DETECTED_PER_VALUE)
                    }
                    _ => None,
                };
                if let Some(mut hint) = hinted {
                    hint.field = path.clone();
                    hints.push(hint);
                }
            }
            label
        }
    };
    FieldEntry {
        path,
        types: sample.types,
        presence: (sample.presence * 100.0).round() / 100.0,
        pseudonymized,
        policy: decision.map(|decision| match decision {
            PolicyDecision::Clear => "clear",
            PolicyDecision::Personal(_) => "personal",
        }),
    }
}
