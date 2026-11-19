use std::collections::HashSet;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PersonalNames {
    known: HashSet<String>,
}

impl PersonalNames {
    pub fn insert(&mut self, name: &str) {
        self.known.insert(name.to_string());
    }

    pub fn contains(&self, name: &str) -> bool {
        self.known.contains(name)
    }
}
