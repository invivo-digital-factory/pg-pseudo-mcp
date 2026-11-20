use std::collections::BTreeSet;

use sqlparser::ast::{Statement, ValueWithSpan};

use super::literal_text::{candidate, literal_text, pattern_parts};
use super::literal_walker::{LiteralSink, walk_literals};

pub fn collect_literals(statement: &Statement) -> Vec<String> {
    let mut collector = Collector::default();
    let mut scratch = statement.clone();
    walk_literals(&mut scratch, &mut collector);
    collector.texts.into_iter().collect()
}

#[derive(Default)]
struct Collector {
    texts: BTreeSet<String>,
}

impl LiteralSink for Collector {
    fn operand(&mut self, value: &mut ValueWithSpan) {
        if let Some(text) = literal_text(&value.value) {
            self.texts.insert(text);
        }
    }

    fn pattern(&mut self, value: &mut ValueWithSpan) {
        if let Some(text) = literal_text(&value.value) {
            let (_, core, _) = pattern_parts(&text);
            self.texts.insert(core.to_string());
        }
    }

    fn literal(&mut self, value: &mut ValueWithSpan) {
        if let Some(text) = literal_text(&value.value) {
            self.texts.insert(candidate(&text).to_string());
        }
    }
}
