use crate::contexts::querying::domain::constants::catalog::DETECTED_PER_VALUE;
use crate::contexts::querying::domain::types::{ColumnClassification, PolicyDecision};

pub fn policy_pseudonymization(
    class: &ColumnClassification,
    decision: Option<&PolicyDecision>,
    type_name: &str,
) -> Option<String> {
    match decision {
        Some(PolicyDecision::Personal(kind)) => Some(kind.clone()),
        Some(PolicyDecision::Clear) => pseudonymization(
            &ColumnClassification {
                forced_kind: None,
                clear_by_type: class.clear_by_type,
            },
            type_name,
        )
        .filter(|label| label != DETECTED_PER_VALUE),
        None => pseudonymization(class, type_name),
    }
}

pub fn pseudonymization(class: &ColumnClassification, type_name: &str) -> Option<String> {
    if let Some(kind) = &class.forced_kind {
        return Some(kind.clone());
    }
    if class.clear_by_type {
        return None;
    }
    let base = type_name.trim_end_matches("[]");
    match base {
        "inet" | "cidr" => Some("ip_address".to_string()),
        "bytea" => Some("withheld".to_string()),
        _ => Some(DETECTED_PER_VALUE.to_string()),
    }
}
