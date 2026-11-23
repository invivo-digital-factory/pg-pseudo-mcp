use serde::Serialize;

use super::Reason;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ColumnPolicyHint {
    pub column: String,
    pub reason: Reason,
    pub hint: &'static str,
}
