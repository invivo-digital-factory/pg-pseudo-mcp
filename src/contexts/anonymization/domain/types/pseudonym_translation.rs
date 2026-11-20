use super::Kind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PseudonymTranslation {
    pub real: Option<(Kind, String)>,
    pub recognised: Option<Kind>,
}
