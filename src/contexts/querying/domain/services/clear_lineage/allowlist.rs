use sqlparser::ast::{BinaryOperator, DataType, ObjectName, UnaryOperator};

use super::names::name_parts;
use crate::contexts::querying::domain::constants::clear_lineage::{CLEAR_FUNCTIONS, SYSTEM_SCHEMA};

pub fn is_clear_function(name: &ObjectName) -> bool {
    match name_parts(name).as_deref() {
        Some([function]) => CLEAR_FUNCTIONS.contains(&function.as_str()),
        Some([schema, function]) => {
            schema == SYSTEM_SCHEMA && CLEAR_FUNCTIONS.contains(&function.as_str())
        }
        _ => false,
    }
}

pub fn is_clear_binary_operator(operator: &BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::Plus
            | BinaryOperator::Minus
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::StringConcat
            | BinaryOperator::Gt
            | BinaryOperator::Lt
            | BinaryOperator::GtEq
            | BinaryOperator::LtEq
            | BinaryOperator::Eq
            | BinaryOperator::NotEq
            | BinaryOperator::And
            | BinaryOperator::Or
    )
}

pub fn is_clear_unary_operator(operator: &UnaryOperator) -> bool {
    matches!(
        operator,
        UnaryOperator::Plus | UnaryOperator::Minus | UnaryOperator::Not
    )
}

pub fn is_builtin_type(data_type: &DataType) -> bool {
    !matches!(
        data_type,
        DataType::Custom(..) | DataType::Array(_) | DataType::Unspecified
    )
}
