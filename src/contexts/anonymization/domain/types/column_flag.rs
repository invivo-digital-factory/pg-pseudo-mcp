use serde::Serialize;

use super::Reason;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ColumnFlag {
    pub kind: String,
    pub why: Reason,
}
