use std::ops::ControlFlow;

use sqlparser::ast::{
    Expr, ObjectName, Query, Select, SetExpr, Statement, TableFactor, UtilityOption, Visit, Visitor,
};

use crate::contexts::querying::domain::constants::sql_policy::{
    ALLOWED_CATALOG_RELATIONS, ALLOWED_INFORMATION_SCHEMA, ALLOWED_SURFACE, CATALOG_HINT,
    DENIED_FUNCTION_PREFIXES, DENIED_FUNCTIONS,
};
use crate::contexts::querying::domain::errors::Refusal;

pub fn validate(statement: &Statement) -> Result<(), Refusal> {
    match statement {
        Statement::Query(_) => {}
        Statement::Explain {
            analyze,
            statement,
            options,
            ..
        } => {
            if *analyze || options.as_deref().is_some_and(requests_analyze) {
                return Err(refused(
                    "explain_analyze",
                    "EXPLAIN ANALYZE executes the statement and is not allowed",
                )
                .with_hint("use the explain tool, which shows the plan without executing"));
            }
            if !matches!(statement.as_ref(), Statement::Query(_)) {
                return Err(not_allowed(statement));
            }
        }
        other => return Err(not_allowed(other)),
    }
    match statement.visit(&mut Checker) {
        ControlFlow::Break(error) => Err(error),
        ControlFlow::Continue(()) => Ok(()),
    }
}

struct Checker;

impl Visitor for Checker {
    type Break = Refusal;

    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<Refusal> {
        if contains_write(&query.body) {
            return ControlFlow::Break(refused(
                "writable_cte",
                "data-modifying statements inside a query are not allowed",
            ));
        }
        if !query.locks.is_empty() {
            return ControlFlow::Break(refused(
                "locking_clause",
                "row locking clauses (FOR UPDATE, FOR SHARE) are not allowed",
            ));
        }
        if let Some(error) = check_table_shorthand(&query.body) {
            return ControlFlow::Break(error);
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_select(&mut self, select: &Select) -> ControlFlow<Refusal> {
        if select.into.is_some() {
            return ControlFlow::Break(refused(
                "select_into",
                "SELECT INTO creates a table and is not allowed",
            ));
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_relation(&mut self, relation: &ObjectName) -> ControlFlow<Refusal> {
        let (schema, name) = split_name(relation);
        if let Some(error) = check_function(&name) {
            return ControlFlow::Break(error);
        }
        if let Some(error) = check_relation(schema.as_deref(), &name) {
            return ControlFlow::Break(error);
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_table_factor(&mut self, factor: &TableFactor) -> ControlFlow<Refusal> {
        let name = match factor {
            TableFactor::Function { name, .. } | TableFactor::SemanticView { name, .. } => name,
            _ => return ControlFlow::Continue(()),
        };
        let (schema, name) = split_name(name);
        if let Some(error) =
            check_function(&name).or_else(|| check_relation(schema.as_deref(), &name))
        {
            return ControlFlow::Break(error);
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<Refusal> {
        if let Expr::Function(function) = expr {
            let (_, name) = split_name(&function.name);
            if let Some(error) = check_function(&name) {
                return ControlFlow::Break(error);
            }
        }
        ControlFlow::Continue(())
    }
}

fn contains_write(body: &SetExpr) -> bool {
    match body {
        SetExpr::Insert(_) | SetExpr::Update(_) | SetExpr::Delete(_) | SetExpr::Merge(_) => true,
        SetExpr::SetOperation { left, right, .. } => contains_write(left) || contains_write(right),
        _ => false,
    }
}

fn check_table_shorthand(body: &SetExpr) -> Option<Refusal> {
    match body {
        SetExpr::Table(table) => {
            let name = table.table_name.as_ref()?.value.to_lowercase();
            let schema = table
                .schema_name
                .as_ref()
                .map(|schema| schema.value.to_lowercase());
            check_function(&name).or_else(|| check_relation(schema.as_deref(), &name))
        }
        SetExpr::SetOperation { left, right, .. } => {
            check_table_shorthand(left).or_else(|| check_table_shorthand(right))
        }
        _ => None,
    }
}

fn requests_analyze(options: &[UtilityOption]) -> bool {
    options.iter().any(|option| {
        (option.name.value.eq_ignore_ascii_case("analyze")
            || option.name.value.eq_ignore_ascii_case("analyse"))
            && !matches!(
                &option.arg,
                Some(Expr::Value(value)) if matches!(value.value, sqlparser::ast::Value::Boolean(false))
            )
            && !matches!(
                &option.arg,
                Some(Expr::Identifier(ident)) if ident.value.eq_ignore_ascii_case("false") || ident.value.eq_ignore_ascii_case("off")
            )
    })
}

fn split_name(name: &ObjectName) -> (Option<String>, String) {
    let parts: Vec<String> = name
        .0
        .iter()
        .map(|part| match part.as_ident() {
            Some(ident) => ident.value.to_lowercase(),
            None => part.to_string().to_lowercase(),
        })
        .collect();
    match parts.as_slice() {
        [] => (None, String::new()),
        [only] => (None, only.clone()),
        [.., schema, last] => (Some(schema.clone()), last.clone()),
    }
}

fn check_function(name: &str) -> Option<Refusal> {
    let denied = DENIED_FUNCTIONS.contains(&name)
        || DENIED_FUNCTION_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix));
    denied.then(|| {
        refused(
            "function_not_allowed",
            format!("function {name} can change server or session state or expose server data"),
        )
        .with_hint(CATALOG_HINT)
    })
}

fn check_relation(schema: Option<&str>, name: &str) -> Option<Refusal> {
    let refused_relation = name.starts_with("pg_stat")
        || match schema {
            Some("pg_catalog") => !ALLOWED_CATALOG_RELATIONS.contains(&name),
            Some("information_schema") => !ALLOWED_INFORMATION_SCHEMA.contains(&name),
            Some(_) => false,
            None => name.starts_with("pg_") && !ALLOWED_CATALOG_RELATIONS.contains(&name),
        };
    refused_relation.then(|| {
        refused(
            "relation_not_allowed",
            format!("catalog relation {name} can expose real values or server secrets"),
        )
        .with_hint(CATALOG_HINT)
    })
}

fn not_allowed(statement: &Statement) -> Refusal {
    let rendered = statement.to_string();
    let keyword = rendered
        .split_whitespace()
        .next()
        .unwrap_or("this")
        .to_uppercase();
    refused(
        "statement_not_allowed",
        format!("{keyword} statements are not allowed"),
    )
    .with_hint(ALLOWED_SURFACE)
}

fn refused(code: &str, message: impl Into<String>) -> Refusal {
    Refusal::new(code, message)
}
