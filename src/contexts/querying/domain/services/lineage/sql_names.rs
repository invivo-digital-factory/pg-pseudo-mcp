use sqlparser::ast::{Ident, ObjectName, TableAlias, TableFactor};

pub fn last_part(name: &ObjectName) -> Option<&Ident> {
    name.0.last().and_then(|part| part.as_ident())
}

pub fn function_name(factor: &TableFactor) -> Vec<String> {
    match factor {
        TableFactor::Function { name, .. } => last_part(name)
            .map(|part| part.value.clone())
            .into_iter()
            .collect(),
        TableFactor::UNNEST { .. } => vec!["unnest".to_string()],
        _ => Vec::new(),
    }
}

pub fn factor_alias(factor: &TableFactor) -> Option<&TableAlias> {
    match factor {
        TableFactor::Table { alias, .. }
        | TableFactor::Derived { alias, .. }
        | TableFactor::TableFunction { alias, .. }
        | TableFactor::Function { alias, .. }
        | TableFactor::UNNEST { alias, .. }
        | TableFactor::JsonTable { alias, .. }
        | TableFactor::OpenJsonTable { alias, .. }
        | TableFactor::NestedJoin { alias, .. }
        | TableFactor::Pivot { alias, .. }
        | TableFactor::Unpivot { alias, .. }
        | TableFactor::MatchRecognize { alias, .. }
        | TableFactor::XmlTable { alias, .. }
        | TableFactor::SemanticView { alias, .. } => alias.as_ref(),
        _ => None,
    }
}
