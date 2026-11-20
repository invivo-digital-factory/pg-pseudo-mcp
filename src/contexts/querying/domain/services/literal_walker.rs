use std::ops::ControlFlow;

use sqlparser::ast::{
    BinaryOperator, Expr, InTarget, Statement, ValueWithSpan, VisitMut, VisitorMut,
};

pub trait LiteralSink {
    fn operand(&mut self, value: &mut ValueWithSpan);

    fn pattern(&mut self, value: &mut ValueWithSpan);

    fn literal(&mut self, value: &mut ValueWithSpan);
}

pub fn walk_literals<S: LiteralSink>(statement: &mut Statement, sink: &mut S) {
    let mut walker = LiteralWalker { sink };
    let _ = statement.visit(&mut walker);
}

struct LiteralWalker<'a, S: LiteralSink> {
    sink: &'a mut S,
}

impl<S: LiteralSink> VisitorMut for LiteralWalker<'_, S> {
    type Break = ();

    fn pre_visit_expr(&mut self, expr: &mut Expr) -> ControlFlow<()> {
        match expr {
            Expr::BinaryOp { left, op, right } if is_equality(op) => {
                self.operand_against_column(left, right);
            }
            Expr::IsDistinctFrom(left, right) | Expr::IsNotDistinctFrom(left, right) => {
                self.operand_against_column(left, right);
            }
            Expr::In {
                expr,
                target: InTarget::List(list),
                ..
            } if is_column(expr) => {
                for item in list {
                    self.operand(item);
                }
            }
            Expr::Like { expr, pattern, .. }
            | Expr::ILike { expr, pattern, .. }
            | Expr::SimilarTo { expr, pattern, .. }
                if is_column(expr) =>
            {
                self.pattern(pattern)
            }
            Expr::AnyOp {
                left,
                compare_op,
                right,
                ..
            } if is_equality(compare_op) && is_column(left) => self.array(right),
            _ => {}
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_value(&mut self, value: &mut ValueWithSpan) -> ControlFlow<()> {
        self.sink.literal(value);
        ControlFlow::Continue(())
    }
}

impl<S: LiteralSink> LiteralWalker<'_, S> {
    fn operand_against_column(&mut self, left: &mut Expr, right: &mut Expr) {
        if is_column(left) {
            self.operand(right);
        } else if is_column(right) {
            self.operand(left);
        }
    }

    fn operand(&mut self, expr: &mut Expr) {
        if let Some(value) = literal_slot(expr) {
            self.sink.operand(value);
        }
    }

    fn pattern(&mut self, expr: &mut Expr) {
        if let Some(value) = literal_slot(expr) {
            self.sink.pattern(value);
        }
    }

    fn array(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Array(array) => {
                for element in &mut array.elem {
                    self.operand(element);
                }
            }
            Expr::Nested(inner) | Expr::Cast { expr: inner, .. } => self.array(inner),
            other => self.operand(other),
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

fn is_equality(op: &BinaryOperator) -> bool {
    matches!(op, BinaryOperator::Eq | BinaryOperator::NotEq)
}

fn is_column(expr: &Expr) -> bool {
    match expr {
        Expr::Identifier(_) | Expr::CompoundIdentifier(_) => true,
        Expr::Nested(inner) | Expr::Cast { expr: inner, .. } => is_column(inner),
        Expr::JsonAccess { value, .. } => is_column(value),
        Expr::BinaryOp {
            left,
            op: BinaryOperator::Arrow | BinaryOperator::LongArrow,
            ..
        } => is_column(left),
        _ => false,
    }
}
