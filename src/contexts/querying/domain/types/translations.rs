use std::collections::{HashMap, HashSet};

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

    pub fn real(&self, text: &str) -> Option<&str> {
        self.real.get(text).map(String::as_str)
    }

    pub fn looks_like_pseudonym(&self, text: &str) -> bool {
        self.shaped.contains(text)
    }
}
