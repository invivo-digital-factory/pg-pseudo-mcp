use std::ops::ControlFlow;

use sqlparser::ast::{Statement, Value, ValueWithSpan, VisitMut, VisitorMut};

use crate::guard::PseudonymLookup;
use crate::tools::result::Warning;

pub struct Outcome {
    pub injected: Vec<(String, String)>,
    pub warnings: Vec<Warning>,
}

pub fn rewrite(statement: &mut Statement, lookup: &dyn PseudonymLookup) -> Outcome {
    let mut rewriter = Rewriter {
        lookup,
        injected: Vec::new(),
        warnings: Vec::new(),
    };
    let _ = statement.visit(&mut rewriter);
    Outcome {
        injected: rewriter.injected,
        warnings: rewriter.warnings,
    }
}

struct Rewriter<'a> {
    lookup: &'a dyn PseudonymLookup,
    injected: Vec<(String, String)>,
    warnings: Vec<Warning>,
}

impl VisitorMut for Rewriter<'_> {
    type Break = ();

    fn pre_visit_value(&mut self, value: &mut ValueWithSpan) -> ControlFlow<()> {
        let Some(text) = literal_text(&value.value) else {
            return ControlFlow::Continue(());
        };
        let numeric = matches!(value.value, Value::Number(..));
        if let Some((_, real)) = self.lookup.reverse(&text) {
            value.value = if numeric && real.chars().all(|c| c.is_ascii_digit()) {
                Value::Number(real.clone(), false)
            } else {
                Value::SingleQuotedString(real.clone())
            };
            self.injected.push((real, text));
            return ControlFlow::Continue(());
        }
        if let Some((prefix, core, suffix)) = like_core(&text)
            && let Some((_, real)) = self.lookup.reverse(core)
        {
            value.value =
                Value::SingleQuotedString(format!("{prefix}{}{suffix}", escape_like(&real)));
            self.injected.push((real, core.to_string()));
            return ControlFlow::Continue(());
        }
        let candidate = like_core(&text).map_or(text.as_str(), |(_, core, _)| core);
        if self.lookup.looks_like_pseudonym(candidate).is_some() {
            self.warnings.push(Warning::new(
                "unresolved_pseudonym",
                format!(
                    "'{candidate}' looks like a pseudonym but is not known on this machine; it was compared as written"
                ),
            ));
        }
        ControlFlow::Continue(())
    }
}

fn literal_text(value: &Value) -> Option<String> {
    match value {
        Value::SingleQuotedString(text)
        | Value::EscapedStringLiteral(text)
        | Value::UnicodeStringLiteral(text)
        | Value::NationalStringLiteral(text)
        | Value::TripleSingleQuotedString(text)
        | Value::TripleDoubleQuotedString(text) => Some(text.clone()),
        Value::DollarQuotedString(dollar) => Some(dollar.value.clone()),
        Value::Number(digits, _) => Some(digits.to_string()),
        _ => None,
    }
}

fn like_core(text: &str) -> Option<(&str, &str, &str)> {
    let start = text.len() - text.trim_start_matches('%').len();
    let end = text.trim_end_matches('%').len();
    if (start == 0 && end == text.len()) || start >= end {
        return None;
    }
    Some((&text[..start], &text[start..end], &text[end..]))
}

fn escape_like(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(character, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}
