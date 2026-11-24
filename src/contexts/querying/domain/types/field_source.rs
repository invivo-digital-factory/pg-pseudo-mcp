#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldSource {
    pub database: String,
    pub collection: String,
    pub path: String,
}
