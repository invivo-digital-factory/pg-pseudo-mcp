use serde::Serialize;

use super::Reason;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldPolicyHint {
    pub collection: String,
    pub field: String,
    pub reason: Reason,
    pub hint: &'static str,
}
