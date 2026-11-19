use crate::contexts::anonymization::use_cases::translate_pseudonym::TranslatePseudonym;
use crate::contexts::querying::domain::repositories::PseudonymTranslator;

pub struct PseudonymTranslatorAdapter {
    translate_pseudonym: TranslatePseudonym,
}

impl PseudonymTranslatorAdapter {
    pub fn new(translate_pseudonym: TranslatePseudonym) -> Self {
        Self {
            translate_pseudonym,
        }
    }
}

impl PseudonymTranslator for PseudonymTranslatorAdapter {
    fn real_value(&self, text: &str) -> Option<String> {
        self.translate_pseudonym
            .execute(text)
            .real
            .map(|(_, real)| real)
    }

    fn looks_like_pseudonym(&self, text: &str) -> bool {
        self.translate_pseudonym.execute(text).recognised.is_some()
    }
}
