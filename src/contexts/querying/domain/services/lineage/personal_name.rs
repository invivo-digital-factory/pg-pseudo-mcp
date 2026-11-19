use std::cell::RefCell;
use std::collections::BTreeSet;

use crate::contexts::querying::domain::types::PersonalNames;

const MIN_PLURAL_STEM_CHARS: usize = 3;

pub trait NameJudge {
    fn knows(&self, name: &str) -> bool;
}

impl NameJudge for PersonalNames {
    fn knows(&self, name: &str) -> bool {
        self.contains(name)
    }
}

#[derive(Default)]
pub struct NameRecorder {
    asked: RefCell<BTreeSet<String>>,
}

impl NameRecorder {
    pub fn into_names(self) -> Vec<String> {
        self.asked.into_inner().into_iter().collect()
    }
}

impl NameJudge for NameRecorder {
    fn knows(&self, name: &str) -> bool {
        self.asked.borrow_mut().insert(name.to_string());
        false
    }
}

pub fn is_personal_name(name: &str, judge: &dyn NameJudge) -> bool {
    if name.is_empty() {
        return false;
    }
    let singular: Vec<&str> = name
        .split('_')
        .map(|part| match part.strip_suffix('s') {
            Some(stem) if stem.chars().count() >= MIN_PLURAL_STEM_CHARS => stem,
            _ => part,
        })
        .collect();
    judge.knows(name) || judge.knows(&singular.join("_"))
}
