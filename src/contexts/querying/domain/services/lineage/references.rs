use std::ops::ControlFlow;

use sqlparser::ast::{
    BinaryOperator, Expr, Function, FunctionArg, FunctionArgExpr, FunctionArguments, ObjectName,
    Value, ValueWithSpan, Visit, Visitor,
};

use super::personal_name::is_personal_name;
use super::scope::Scope;
use super::sql_names::last_part;

pub fn references<V: Visit>(node: &V, scope: Scope<'_>) -> bool {
    struct Finder<'a> {
        scope: Scope<'a>,
    }

    impl Visitor for Finder<'_> {
        type Break = ();

        fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<()> {
            let found = match expr {
                Expr::Identifier(ident) => self.scope.is_tainted(ident),
                Expr::CompoundIdentifier(parts) => parts
                    .last()
                    .is_some_and(|ident| self.scope.is_tainted(ident)),
                Expr::QualifiedWildcard(name, _) => {
                    last_part(name).is_some_and(|part| self.scope.is_tainted(part))
                }
                Expr::Function(function) if expands_tainted_row(function, self.scope) => true,
                Expr::BinaryOp {
                    op:
                        BinaryOperator::Arrow
                        | BinaryOperator::LongArrow
                        | BinaryOperator::HashArrow
                        | BinaryOperator::HashLongArrow,
                    right,
                    ..
                } => has_personal_key(right.as_ref(), self.scope),
                Expr::Function(function) if is_json_path_function(&function.name) => {
                    has_personal_key(function, self.scope)
                }
                _ => false,
            };
            if found {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        }
    }

    node.visit(&mut Finder { scope }).is_break()
}

fn expands_tainted_row(function: &Function, scope: Scope<'_>) -> bool {
    let FunctionArguments::List(list) = &function.args else {
        return false;
    };
    list.args.iter().any(|arg| {
        let expr = match arg {
            FunctionArg::Named { arg, .. }
            | FunctionArg::ExprNamed { arg, .. }
            | FunctionArg::Unnamed(arg) => arg,
        };
        matches!(expr, FunctionArgExpr::QualifiedWildcard(name)
            if last_part(name).is_some_and(|part| scope.is_tainted(part)))
    })
}

fn has_personal_key<V: Visit>(node: &V, scope: Scope<'_>) -> bool {
    struct Keys<'a> {
        scope: Scope<'a>,
    }

    impl Visitor for Keys<'_> {
        type Break = ();

        fn pre_visit_value(&mut self, value: &ValueWithSpan) -> ControlFlow<()> {
            let text = match &value.value {
                Value::SingleQuotedString(text)
                | Value::EscapedStringLiteral(text)
                | Value::UnicodeStringLiteral(text) => text.as_str(),
                Value::DollarQuotedString(dollar) => dollar.value.as_str(),
                _ => return ControlFlow::Continue(()),
            };
            let personal = text
                .split(|c: char| matches!(c, ',' | '{' | '}' | '"') || c.is_whitespace())
                .any(|name| is_personal_name(name, self.scope.judge));
            if personal {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        }
    }

    node.visit(&mut Keys { scope }).is_break()
}

fn is_json_path_function(name: &ObjectName) -> bool {
    last_part(name).is_some_and(|part| {
        let lower = part.value.to_lowercase();
        lower.starts_with("json") && lower.contains("extract_path")
    })
}
