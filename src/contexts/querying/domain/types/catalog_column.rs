#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogColumn {
    pub schema: String,
    pub table: String,
    pub name: String,
    pub type_name: String,
}
