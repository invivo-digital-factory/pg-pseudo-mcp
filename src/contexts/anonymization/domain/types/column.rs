use super::Kind;

#[derive(Debug, Clone, Copy)]
pub struct ColumnInfo<'a> {
    pub column_name: Option<&'a str>,
    pub type_name: &'a str,
    pub derived: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnClass {
    pub forced_kind: Option<Kind>,
    pub clear_by_type: bool,
    pub derived: bool,
    pub structural_only: bool,
    pub forced_by_policy: bool,
}
