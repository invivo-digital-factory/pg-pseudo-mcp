use std::ops::ControlFlow;

use sqlparser::ast::{
    BinaryOperator, Expr, InTarget, Statement, Value, ValueWithSpan, VisitMut, VisitorMut,
};

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

    fn pre_visit_expr(&mut self, expr: &mut Expr) -> ControlFlow<()> {
        match expr {
            Expr::BinaryOp { left, op, right } if is_comparison(op) => {
                self.translate_operand(left);
                self.translate_operand(right);
            }
            Expr::IsDistinctFrom(left, right) | Expr::IsNotDistinctFrom(left, right) => {
                self.translate_operand(left);
                self.translate_operand(right);
            }
            Expr::In {
                target: InTarget::List(list),
                ..
            } => {
                for item in list {
                    self.translate_operand(item);
                }
            }
            Expr::Between { low, high, .. } => {
                self.translate_operand(low);
                self.translate_operand(high);
            }
            Expr::Like { pattern, .. }
            | Expr::ILike { pattern, .. }
            | Expr::SimilarTo { pattern, .. } => self.translate_pattern(pattern),
            Expr::AnyOp { right, .. } | Expr::AllOp { right, .. } => self.translate_array(right),
            _ => {}
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_value(&mut self, value: &mut ValueWithSpan) -> ControlFlow<()> {
        let Some(text) = literal_text(&value.value) else {
            return ControlFlow::Continue(());
        };
        let candidate = like_core(&text).map_or(text.as_str(), |(_, core, _)| core);
        if self.lookup.reverse(candidate).is_some() {
            self.warnings.push(Warning::new(
                "pseudonym_not_translated",
                format!(
                    "'{candidate}' is a pseudonym used outside a comparison; it is translated only when compared with a column (=, <>, IN, ANY, LIKE, BETWEEN)"
                ),
            ));
        } else if self.lookup.looks_like_pseudonym(candidate).is_some() {
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

impl Rewriter<'_> {
    fn translate_operand(&mut self, expr: &mut Expr) {
        let Some(value) = literal_slot(expr) else {
            return;
        };
        let Some(text) = literal_text(&value.value) else {
            return;
        };
        let Some((_, real)) = self.lookup.reverse(&text) else {
            return;
        };
        let numeric = matches!(value.value, Value::Number(..));
        value.value = if numeric && real.chars().all(|c| c.is_ascii_digit()) {
            Value::Number(real.clone(), false)
        } else {
            Value::SingleQuotedString(real.clone())
        };
        self.injected.push((real, text));
    }

    fn translate_pattern(&mut self, expr: &mut Expr) {
        let Some(value) = literal_slot(expr) else {
            return;
        };
        let Some(text) = literal_text(&value.value) else {
            return;
        };
        let (prefix, core, suffix) = like_core(&text).unwrap_or(("", text.as_str(), ""));
        let Some((_, real)) = self.lookup.reverse(core) else {
            return;
        };
        let replacement = format!("{prefix}{}{suffix}", escape_like(&real));
        self.injected.push((real, core.to_string()));
        value.value = Value::SingleQuotedString(replacement);
    }

    fn translate_array(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Array(array) => {
                for element in &mut array.elem {
                    self.translate_operand(element);
                }
            }
            Expr::Nested(inner) | Expr::Cast { expr: inner, .. } => self.translate_array(inner),
            other => self.translate_operand(other),
        }
    }
}

fn literal_slot(expr: &mut Expr) -> Option<&mut ValueWithSpan> {
    match expr {
        Expr::Value(value) => Some(value),
        Expr::Nested(inner) | Expr::Cast { expr: inner, .. } => literal_slot(inner),
        _ => None,
    }
}

fn is_comparison(op: &BinaryOperator) -> bool {
    matches!(
        op,
        BinaryOperator::Eq
            | BinaryOperator::NotEq
            | BinaryOperator::Lt
            | BinaryOperator::LtEq
            | BinaryOperator::Gt
            | BinaryOperator::GtEq
    )
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
