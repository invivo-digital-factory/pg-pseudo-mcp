#[derive(Debug, Clone, PartialEq)]
pub struct SampledField {
    pub path: Vec<String>,
    pub types: Vec<String>,
    pub presence: f64,
}
