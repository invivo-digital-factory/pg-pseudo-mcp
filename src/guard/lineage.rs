use std::collections::HashSet;
use std::ops::ControlFlow;

use sqlparser::ast::{
    BinaryOperator, Expr, Ident, ObjectName, Query, Select, SelectItem, SetExpr, Statement,
    TableAlias, TableFactor, Value, ValueWithSpan, Visit, Visitor,
};

use crate::anon::dictionary_kind;

pub fn derived_lineage(statement: &Statement) -> Option<Vec<bool>> {
    let Statement::Query(query) = statement else {
        return None;
    };
    let tainted = tainted_names(statement);
    let shape = Shape::of_query(query, &tainted);
    (!shape.rest).then_some(shape.items)
}

fn tainted_names(statement: &Statement) -> HashSet<String> {
    let mut collector = Collector::default();
    loop {
        collector.changed = false;
        let _ = statement.visit(&mut collector);
        if !collector.changed {
            return collector.tainted;
        }
    }
}

#[derive(Default)]
struct Collector {
    tainted: HashSet<String>,
    changed: bool,
}

impl Collector {
    fn taint(&mut self, names: impl IntoIterator<Item = String>) {
        for name in names {
            if self.tainted.insert(name.to_lowercase()) {
                self.changed = true;
            }
        }
    }

    fn taint_alias(&mut self, alias: &TableAlias, shape: &Shape) {
        let mut names = Vec::new();
        if shape.any() {
            names.push(alias.name.value.clone());
        }
        for (index, column) in alias.columns.iter().enumerate() {
            if shape.at(index) {
                names.push(column.name.value.clone());
            }
        }
        self.taint(names);
    }
}

impl Visitor for Collector {
    type Break = ();

    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<()> {
        if let Some(with) = &query.with {
            for cte in &with.cte_tables {
                let shape = Shape::of_query(&cte.query, &self.tainted);
                self.taint_alias(&cte.alias, &shape);
            }
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_select(&mut self, select: &Select) -> ControlFlow<()> {
        let mut names = Vec::new();
        for item in &select.projection {
            match item {
                SelectItem::ExprWithAlias { expr, alias } if references(expr, &self.tainted) => {
                    names.push(alias.value.clone());
                }
                SelectItem::ExprWithAliases { expr, aliases }
                    if references(expr, &self.tainted) =>
                {
                    names.extend(aliases.iter().map(|alias| alias.value.clone()));
                }
                SelectItem::UnnamedExpr(expr) if references(expr, &self.tainted) => {
                    names.extend(implicit_names(expr));
                }
                _ => {}
            }
        }
        self.taint(names);
        ControlFlow::Continue(())
    }

    fn pre_visit_table_factor(&mut self, factor: &TableFactor) -> ControlFlow<()> {
        match factor {
            TableFactor::Derived {
                subquery,
                alias: Some(alias),
                ..
            } => {
                let shape = Shape::of_query(subquery, &self.tainted);
                self.taint_alias(alias, &shape);
            }
            TableFactor::Derived { .. } => {}
            TableFactor::Table {
                name, alias, args, ..
            } => {
                let from_function = args
                    .as_ref()
                    .is_some_and(|args| references(args, &self.tainted));
                let from_relation =
                    last_part(name).is_some_and(|part| is_tainted(part, &self.tainted));
                if from_function {
                    self.taint(last_part(name).map(|part| part.value.clone()));
                }
                if let Some(alias) = alias {
                    let mut names: Vec<String> = alias
                        .columns
                        .iter()
                        .map(|column| column.name.value.clone())
                        .collect();
                    if from_function || from_relation {
                        names.push(alias.name.value.clone());
                    }
                    self.taint(names);
                }
            }
            other => {
                if references(other, &self.tainted) {
                    self.taint(function_name(other));
                    if let Some(alias) = factor_alias(other) {
                        self.taint_alias(alias, &Shape::all());
                    }
                }
            }
        }
        ControlFlow::Continue(())
    }
}

#[derive(Debug, Default)]
struct Shape {
    items: Vec<bool>,
    rest: bool,
}

impl Shape {
    fn all() -> Self {
        Self {
            items: Vec::new(),
            rest: true,
        }
    }

    fn any(&self) -> bool {
        self.rest || self.items.iter().any(|item| *item)
    }

    fn at(&self, index: usize) -> bool {
        self.items.get(index).copied().unwrap_or(self.rest)
    }

    fn of_query(query: &Query, tainted: &HashSet<String>) -> Self {
        Self::of_set_expr(&query.body, tainted)
    }

    fn of_set_expr(body: &SetExpr, tainted: &HashSet<String>) -> Self {
        match body {
            SetExpr::Select(select) => Self::of_projection(&select.projection, tainted),
            SetExpr::Query(query) => Self::of_query(query, tainted),
            SetExpr::SetOperation { left, right, .. } => {
                Self::of_set_expr(left, tainted).merge(Self::of_set_expr(right, tainted))
            }
            SetExpr::Values(values) => values
                .rows
                .iter()
                .map(|row| Self {
                    items: row
                        .content
                        .iter()
                        .map(|expr| references(expr, tainted))
                        .collect(),
                    rest: false,
                })
                .reduce(Self::merge)
                .unwrap_or_default(),
            _ => Self::all(),
        }
    }

    fn of_projection(projection: &[SelectItem], tainted: &HashSet<String>) -> Self {
        let mut items = Vec::with_capacity(projection.len());
        for item in projection {
            match item {
                SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. }
                    if !is_wildcard(expr) =>
                {
                    items.push(references(expr, tainted));
                }
                SelectItem::ExprWithAliases { expr, aliases } => {
                    let flag = references(expr, tainted);
                    items.extend(aliases.iter().map(|_| flag));
                }
                _ => {
                    return Self { items, rest: true };
                }
            }
        }
        Self { items, rest: false }
    }

    fn merge(self, other: Self) -> Self {
        let len = self.items.len().max(other.items.len());
        Self {
            items: (0..len)
                .map(|index| self.at(index) || other.at(index))
                .collect(),
            rest: self.rest || other.rest,
        }
    }
}

fn is_wildcard(expr: &Expr) -> bool {
    match expr {
        Expr::Wildcard(_) | Expr::QualifiedWildcard(..) => true,
        Expr::Nested(inner) => is_wildcard(inner),
        _ => false,
    }
}

fn references<V: Visit>(node: &V, tainted: &HashSet<String>) -> bool {
    struct Finder<'a> {
        tainted: &'a HashSet<String>,
    }

    impl Visitor for Finder<'_> {
        type Break = ();

        fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<()> {
            let found = match expr {
                Expr::Identifier(ident) => is_tainted(ident, self.tainted),
                Expr::CompoundIdentifier(parts) => parts
                    .last()
                    .is_some_and(|ident| is_tainted(ident, self.tainted)),
                Expr::BinaryOp {
                    op:
                        BinaryOperator::Arrow
                        | BinaryOperator::LongArrow
                        | BinaryOperator::HashArrow
                        | BinaryOperator::HashLongArrow,
                    right,
                    ..
                } => has_personal_key(right.as_ref()),
                Expr::Function(function) if is_json_path_function(&function.name) => {
                    has_personal_key(function)
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

    node.visit(&mut Finder { tainted }).is_break()
}

fn has_personal_key<V: Visit>(node: &V) -> bool {
    struct Keys;

    impl Visitor for Keys {
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
                .any(is_personal_name);
            if personal {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        }
    }

    node.visit(&mut Keys).is_break()
}

fn is_json_path_function(name: &ObjectName) -> bool {
    last_part(name).is_some_and(|part| {
        let lower = part.value.to_lowercase();
        lower.starts_with("json") && lower.contains("extract_path")
    })
}

fn is_tainted(ident: &Ident, tainted: &HashSet<String>) -> bool {
    is_personal_name(&ident.value) || tainted.contains(&ident.value.to_lowercase())
}

fn is_personal_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let singular: Vec<&str> = name
        .split('_')
        .map(|part| match part.strip_suffix('s') {
            Some(stem) if stem.chars().count() >= MIN_PLURAL_STEM_CHARS => stem,
            _ => part,
        })
        .collect();
    dictionary_kind(name).is_some() || dictionary_kind(&singular.join("_")).is_some()
}

const MIN_PLURAL_STEM_CHARS: usize = 3;

fn implicit_names(expr: &Expr) -> Vec<String> {
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

fn last_part(name: &ObjectName) -> Option<&Ident> {
    name.0.last().and_then(|part| part.as_ident())
}

fn function_name(factor: &TableFactor) -> Vec<String> {
    match factor {
        TableFactor::Function { name, .. } => last_part(name)
            .map(|part| part.value.clone())
            .into_iter()
            .collect(),
        TableFactor::UNNEST { .. } => vec!["unnest".to_string()],
        _ => Vec::new(),
    }
}

fn factor_alias(factor: &TableFactor) -> Option<&TableAlias> {
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
