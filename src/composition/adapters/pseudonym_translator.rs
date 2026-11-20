use crate::contexts::anonymization::use_cases::translate_pseudonym::TranslatePseudonym;
use crate::contexts::querying::domain::repositories::PseudonymTranslator;
use crate::contexts::querying::domain::types::PseudonymLookup;

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
    fn lookup(&self, text: &str) -> PseudonymLookup {
        let translation = self.translate_pseudonym.execute(text);
        match (translation.real, translation.recognised) {
            (Some((_, real)), _) => PseudonymLookup::Known { real },
            (None, Some(_)) => PseudonymLookup::Unknown,
            (None, None) => PseudonymLookup::NotAPseudonym,
        }
    }
}
