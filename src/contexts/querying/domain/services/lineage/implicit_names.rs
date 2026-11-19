use sqlparser::ast::{Expr, SelectItem, SetExpr};

use super::sql_names::last_part;

pub fn implicit_names(expr: &Expr) -> Vec<String> {
    match expr {
        Expr::Identifier(ident) => vec![ident.value.clone()],
        Expr::CompoundIdentifier(parts) => parts
            .last()
            .map(|part| part.value.clone())
            .into_iter()
            .collect(),
        Expr::Function(function) => last_part(&function.name)
            .map(|part| part.value.clone())
            .into_iter()
            .collect(),
        Expr::Nested(inner) | Expr::Collate { expr: inner, .. } => implicit_names(inner),
        Expr::CompoundFieldAccess { root, .. } => {
            let mut names = implicit_names(root);
            names.push(UNNAMED.to_string());
            names
        }
        Expr::Cast {
            expr: inner,
            data_type,
            ..
        } => {
            let mut names = implicit_names(inner);
            let written = data_type.to_string().to_lowercase();
            let base = written.split('(').next().unwrap_or(&written).trim();
            names.push(base.to_string());
            names.extend(internal_type_name(base).map(str::to_string));
            names
        }
        Expr::Case { .. } => vec!["case".to_string()],
        Expr::Array(_) => vec!["array".to_string()],
        Expr::Substring { .. } => vec!["substring".to_string()],
        Expr::Trim { .. } => vec![
            "btrim".to_string(),
            "ltrim".to_string(),
            "rtrim".to_string(),
        ],
        Expr::Overlay { .. } => vec!["overlay".to_string()],
        Expr::Position { .. } => vec!["position".to_string()],
        Expr::Subquery(query) => match query.body.as_ref() {
            SetExpr::Select(select) => match select.projection.first() {
                Some(SelectItem::ExprWithAlias { alias, .. }) => vec![alias.value.clone()],
                Some(SelectItem::UnnamedExpr(inner)) => implicit_names(inner),
                _ => vec![UNNAMED.to_string()],
            },
            _ => vec![UNNAMED.to_string()],
        },
        _ => vec![UNNAMED.to_string()],
    }
}

const UNNAMED: &str = "?column?";

fn internal_type_name(written: &str) -> Option<&'static str> {
    match written {
        "int" | "integer" => Some("int4"),
        "bigint" => Some("int8"),
        "smallint" => Some("int2"),
        "real" => Some("float4"),
        "double precision" => Some("float8"),
        "boolean" => Some("bool"),
        "character varying" => Some("varchar"),
        "char" | "character" => Some("bpchar"),
        "decimal" => Some("numeric"),
        _ => None,
    }
}
