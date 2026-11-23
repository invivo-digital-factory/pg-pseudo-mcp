use sqlparser::ast::{
    Expr, Function, FunctionArg, FunctionArgExpr, FunctionArgumentClause, FunctionArguments,
    InTarget, OrderByExpr, Value, WindowType,
};

use super::allowlist::{
    is_builtin_type, is_clear_binary_operator, is_clear_function, is_clear_unary_operator,
};
use super::relation_scope::RelationScope;

pub fn is_clear(expr: &Expr, scope: &RelationScope<'_>) -> bool {
    let clear = |inner: &Expr| is_clear(inner, scope);
    match expr {
        Expr::Identifier(ident) => scope.is_clear_reference(std::slice::from_ref(ident)),
        Expr::CompoundIdentifier(parts) => scope.is_clear_reference(parts),
        Expr::Value(value) => !matches!(value.value, Value::Placeholder(_)),
        Expr::TypedString(typed) => is_builtin_type(&typed.data_type),
        Expr::Nested(inner) => clear(inner),
        Expr::Cast {
            expr: inner,
            data_type,
            format: None,
            ..
        } => is_builtin_type(data_type) && clear(inner),
        Expr::BinaryOp { left, op, right } => {
            is_clear_binary_operator(op) && clear(left) && clear(right)
        }
        Expr::UnaryOp { op, expr: inner } => is_clear_unary_operator(op) && clear(inner),
        Expr::IsNull(inner)
        | Expr::IsNotNull(inner)
        | Expr::IsTrue(inner)
        | Expr::IsNotTrue(inner)
        | Expr::IsFalse(inner)
        | Expr::IsNotFalse(inner)
        | Expr::IsUnknown(inner)
        | Expr::IsNotUnknown(inner) => clear(inner),
        Expr::IsDistinctFrom(left, right) | Expr::IsNotDistinctFrom(left, right) => {
            clear(left) && clear(right)
        }
        Expr::Between {
            expr: inner,
            low,
            high,
            ..
        } => clear(inner) && clear(low) && clear(high),
        Expr::In {
            expr: inner,
            target: InTarget::List(list),
            ..
        } => clear(inner) && list.iter().all(clear),
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => {
            operand.as_deref().is_none_or(clear)
                && conditions
                    .iter()
                    .all(|when| clear(&when.condition) && clear(&when.result))
                && else_result.as_deref().is_none_or(clear)
        }
        Expr::Trim {
            expr: inner,
            trim_what,
            trim_characters,
            ..
        } => {
            clear(inner)
                && trim_what.as_deref().is_none_or(clear)
                && trim_characters
                    .as_deref()
                    .is_none_or(|characters| characters.iter().all(clear))
        }
        Expr::Extract { expr: inner, .. } => clear(inner),
        Expr::Function(function) => is_clear_function_call(function, scope),
        _ => false,
    }
}

fn is_clear_function_call(function: &Function, scope: &RelationScope<'_>) -> bool {
    let clear = |inner: &Expr| is_clear(inner, scope);
    let FunctionArguments::List(list) = &function.args else {
        return false;
    };
    is_clear_function(&function.name)
        && matches!(function.parameters, FunctionArguments::None)
        && function.within_group.is_empty()
        && function.null_treatment.is_none()
        && function.filter.as_deref().is_none_or(clear)
        && is_clear_window(function.over.as_ref(), scope)
        && list.args.iter().all(|arg| is_clear_argument(arg, scope))
        && list.clauses.iter().all(|clause| match clause {
            FunctionArgumentClause::OrderBy(order) => is_clear_order(order, scope),
            _ => false,
        })
}

fn is_clear_argument(arg: &FunctionArg, scope: &RelationScope<'_>) -> bool {
    match arg {
        FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => is_clear(expr, scope),
        FunctionArg::Unnamed(FunctionArgExpr::Wildcard) => true,
        _ => false,
    }
}

fn is_clear_window(over: Option<&WindowType>, scope: &RelationScope<'_>) -> bool {
    match over {
        None => true,
        Some(WindowType::WindowSpec(spec)) => {
            spec.window_name.is_none()
                && spec.partition_by.iter().all(|expr| is_clear(expr, scope))
                && is_clear_order(&spec.order_by, scope)
        }
        Some(WindowType::NamedWindow(_)) => false,
    }
}

fn is_clear_order(order: &[OrderByExpr], scope: &RelationScope<'_>) -> bool {
    order.iter().all(|item| is_clear(&item.expr, scope))
}
