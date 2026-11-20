use crate::contexts::querying::domain::constants::catalog::DETECTED_PER_VALUE;
use crate::contexts::querying::domain::types::ColumnClassification;

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
