pub trait PseudonymTranslator: Send + Sync {
    fn real_value(&self, text: &str) -> Option<String>;

    fn looks_like_pseudonym(&self, text: &str) -> bool;
}
