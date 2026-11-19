use crate::contexts::anonymization::domain::types::Kind;
use crate::contexts::anonymization::injector::AnonymizationUseCases;
use crate::guard::PseudonymLookup;

impl PseudonymLookup for AnonymizationUseCases {
    fn reverse(&self, text: &str) -> Option<(Kind, String)> {
        self.translate_pseudonym.execute(text).real
    }

    fn looks_like_pseudonym(&self, text: &str) -> Option<Kind> {
        self.translate_pseudonym.execute(text).recognised
    }
}
