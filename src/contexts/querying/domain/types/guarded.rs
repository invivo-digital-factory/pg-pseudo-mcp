use super::{Lineage, Warning};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guarded {
    pub sql: String,
    pub translated_literals: usize,
    pub injected: Vec<(String, String)>,
    pub warnings: Vec<Warning>,
    pub derived_lineage: Option<Vec<Lineage>>,
    pub is_explain: bool,
}

impl Guarded {
    pub fn explain_sql(&self) -> String {
        if self.is_explain {
            self.sql.clone()
        } else {
            format!("EXPLAIN {}", self.sql)
        }
    }
}
