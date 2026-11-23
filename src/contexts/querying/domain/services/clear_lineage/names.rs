use sqlparser::ast::{Expr, Ident, ObjectName};

pub fn normalized(ident: &Ident) -> String {
    if ident.quote_style.is_some() {
        ident.value.clone()
    } else {
        ident.value.to_lowercase()
    }
}

pub fn name_parts(name: &ObjectName) -> Option<Vec<String>> {
    name.0
        .iter()
        .map(|part| part.as_ident().map(normalized))
        .collect()
}

pub fn output_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(ident) => Some(normalized(ident)),
        Expr::CompoundIdentifier(parts) => parts.last().map(normalized),
        Expr::Nested(inner) | Expr::Cast { expr: inner, .. } => output_name(inner),
        Expr::Function(function) => name_parts(&function.name)?.pop(),
        Expr::Case { .. } => Some("case".to_string()),
        _ => None,
    }
}

pub fn is_wildcard(expr: &Expr) -> bool {
    match expr {
        Expr::Wildcard(_) | Expr::QualifiedWildcard(..) => true,
        Expr::Nested(inner) => is_wildcard(inner),
        _ => false,
    }
}
