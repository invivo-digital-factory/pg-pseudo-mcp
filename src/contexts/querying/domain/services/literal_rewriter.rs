use sqlparser::ast::{Statement, Value, ValueWithSpan};

use super::literal_text::{candidate, escape_like, literal_text, pattern_parts};
use super::literal_walker::{LiteralSink, walk_literals};
use crate::contexts::querying::domain::types::{Translations, Warning};

pub struct Outcome {
    pub injected: Vec<(String, String)>,
    pub warnings: Vec<Warning>,
}

pub fn rewrite_literals(statement: &mut Statement, translations: &Translations) -> Outcome {
    let mut rewriter = Rewriter {
        translations,
        injected: Vec::new(),
        warnings: Vec::new(),
    };
    walk_literals(statement, &mut rewriter);
    Outcome {
        injected: rewriter.injected,
        warnings: rewriter.warnings,
    }
}

struct Rewriter<'a> {
    translations: &'a Translations,
    injected: Vec<(String, String)>,
    warnings: Vec<Warning>,
}

impl LiteralSink for Rewriter<'_> {
    fn operand(&mut self, value: &mut ValueWithSpan) {
        let Some(text) = literal_text(&value.value) else {
            return;
        };
        let Some(real) = self.translations.real(&text) else {
            return;
        };
        let real = real.to_string();
        let numeric = matches!(value.value, Value::Number(..));
        value.value = if numeric && real.chars().all(|c| c.is_ascii_digit()) {
            Value::Number(real.clone(), false)
        } else {
            Value::SingleQuotedString(real.clone())
        };
        self.injected.push((real, text));
    }

    fn pattern(&mut self, value: &mut ValueWithSpan) {
        let Some(text) = literal_text(&value.value) else {
            return;
        };
        let (prefix, core, suffix) = pattern_parts(&text);
        let Some(real) = self.translations.real(core) else {
            return;
        };
        let replacement = format!("{prefix}{}{suffix}", escape_like(real));
        self.injected.push((real.to_string(), core.to_string()));
        value.value = Value::SingleQuotedString(replacement);
    }

    fn literal(&mut self, value: &mut ValueWithSpan) {
        let Some(text) = literal_text(&value.value) else {
            return;
        };
        let candidate = candidate(&text);
        if self.translations.real(candidate).is_some() {
            self.warnings.push(Warning::new(
                "pseudonym_not_translated",
                format!(
                    "'{candidate}' is a pseudonym used outside a comparison with a column; it is translated only in column = value, column <> value, column IN (...), column = ANY(...) and column LIKE pattern"
                ),
            ));
        } else if self.translations.looks_like_pseudonym(candidate) {
            self.warnings.push(Warning::new(
                "unresolved_pseudonym",
                format!(
                    "'{candidate}' looks like a pseudonym but is not known on this machine; it was compared as written"
                ),
            ));
        }
    }
}
