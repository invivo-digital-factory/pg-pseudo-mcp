use std::ops::ControlFlow;

use sqlparser::ast::{Expr, ObjectName, Query, Select, SetExpr, Statement, Visit, Visitor};

use crate::tools::errors::{ErrorCategory, ToolError};

const ALLOWED_SURFACE: &str =
    "only one SELECT, WITH ... SELECT, VALUES or EXPLAIN (without ANALYZE) statement is accepted";

const CATALOG_HINT: &str = "use describe_table, list_tables or search_schema to explore structure; catalog statistics and server views are not readable";

const DENIED_FUNCTION_PREFIXES: &[&str] = &[
    "pg_stat_reset",
    "pg_stat_get_",
    "pg_stat_file",
    "pg_switch_",
    "pg_create_",
    "pg_drop_",
    "pg_rotate_",
    "pg_wal_",
    "pg_replication_",
    "pg_logical_",
    "pg_log_",
    "pg_import_",
    "pg_terminate_",
    "pg_cancel_",
    "pg_sleep",
    "pg_advisory",
    "pg_try_advisory",
    "pg_read_",
    "pg_ls_",
    "pg_file_",
    "lo_",
    "dblink",
    "query_to_xml",
    "table_to_xml",
    "schema_to_xml",
    "database_to_xml",
    "cursor_to_xml",
];

const DENIED_FUNCTIONS: &[&str] = &[
    "pg_promote",
    "pg_reload_conf",
    "set_config",
    "nextval",
    "setval",
    "pg_notify",
];

const ALLOWED_CATALOG_RELATIONS: &[&str] = &[
    "pg_class",
    "pg_attribute",
    "pg_namespace",
    "pg_index",
    "pg_constraint",
    "pg_type",
    "pg_enum",
    "pg_description",
    "pg_proc",
    "pg_views",
    "pg_tables",
    "pg_indexes",
    "pg_inherits",
    "pg_matviews",
];

pub fn validate(statement: &Statement) -> Result<(), ToolError> {
    match statement {
        Statement::Query(_) => {}
        Statement::Explain {
            analyze, statement, ..
        } => {
            if *analyze {
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
    type Break = ToolError;

    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<ToolError> {
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
        ControlFlow::Continue(())
    }

    fn pre_visit_select(&mut self, select: &Select) -> ControlFlow<ToolError> {
        if select.into.is_some() {
            return ControlFlow::Break(refused(
                "select_into",
                "SELECT INTO creates a table and is not allowed",
            ));
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_relation(&mut self, relation: &ObjectName) -> ControlFlow<ToolError> {
        let (schema, name) = split_name(relation);
        if let Some(error) = check_function(&name) {
            return ControlFlow::Break(error);
        }
        if let Some(error) = check_relation(schema.as_deref(), &name) {
            return ControlFlow::Break(error);
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<ToolError> {
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

fn check_function(name: &str) -> Option<ToolError> {
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

fn check_relation(schema: Option<&str>, name: &str) -> Option<ToolError> {
    let catalog = match schema {
        Some("pg_catalog") => true,
        Some(_) => false,
        None => name.starts_with("pg_"),
    };
    (catalog && !ALLOWED_CATALOG_RELATIONS.contains(&name)).then(|| {
        refused(
            "relation_not_allowed",
            format!("catalog relation {name} can expose real values or server secrets"),
        )
        .with_hint(CATALOG_HINT)
    })
}

fn not_allowed(statement: &Statement) -> ToolError {
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

fn refused(code: &str, message: impl Into<String>) -> ToolError {
    ToolError::new(ErrorCategory::RefusedReadOnly, code, message)
}
