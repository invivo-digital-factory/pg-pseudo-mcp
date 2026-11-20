use std::collections::{HashMap, HashSet};

use super::PseudonymLookup;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Translations {
    real: HashMap<String, String>,
    shaped: HashSet<String>,
}

impl Translations {
    pub fn insert_real(&mut self, pseudonym: &str, real: String) {
        self.real.insert(pseudonym.to_string(), real);
    }

    pub fn insert_shaped(&mut self, text: &str) {
        self.shaped.insert(text.to_string());
    }

    pub fn record(&mut self, text: &str, lookup: PseudonymLookup) {
        match lookup {
            PseudonymLookup::Known { real } => self.insert_real(text, real),
            PseudonymLookup::Unknown => self.insert_shaped(text),
            PseudonymLookup::NotAPseudonym => {}
        }
    }

    pub fn real(&self, text: &str) -> Option<&str> {
        self.real.get(text).map(String::as_str)
    }

    pub fn looks_like_pseudonym(&self, text: &str) -> bool {
        self.shaped.contains(text)
    }
}
