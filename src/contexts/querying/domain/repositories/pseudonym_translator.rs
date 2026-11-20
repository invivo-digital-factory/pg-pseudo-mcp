use crate::contexts::querying::domain::types::PseudonymLookup;

pub trait PseudonymTranslator: Send + Sync {
    fn lookup(&self, text: &str) -> PseudonymLookup;
}
