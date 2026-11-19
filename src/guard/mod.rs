mod lineage;
mod rewrite;
mod validate;

use sqlparser::ast::Statement;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use crate::kind::Kind;
use crate::tools::errors::{ErrorCategory, ToolError};
use crate::tools::result::Warning;

pub trait PseudonymLookup {
    fn reverse(&self, text: &str) -> Option<(Kind, String)>;
    fn looks_like_pseudonym(&self, text: &str) -> Option<Kind>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guarded {
    pub sql: String,
    pub translated_literals: usize,
    pub injected: Vec<(String, String)>,
    pub warnings: Vec<Warning>,
    pub derived_lineage: Option<Vec<bool>>,
    is_explain: bool,
}

impl Guarded {
    pub fn explain_sql(&self) -> String {
        if self.is_explain {
            self.sql.clone()
        } else {
            format!("EXPLAIN {}", self.sql)
        }
    }
}

pub fn prepare(sql: &str, lookup: &dyn PseudonymLookup) -> Result<Guarded, ToolError> {
    let mut statement = parse_single(sql)?;
    validate::validate(&statement)?;
    let outcome = rewrite::rewrite(&mut statement, lookup);
    let rewritten = statement.to_string();
    let reparsed = parse_single(&rewritten).map_err(|_| {
        ToolError::new(
            ErrorCategory::RefusedReadOnly,
            "rewrite_failed",
            "the statement could not be safely re-serialized after translating pseudonyms",
        )
        .with_hint("rewrite the query with simpler syntax")
    })?;
    validate::validate(&reparsed)?;
    Ok(Guarded {
        derived_lineage: lineage::derived_lineage(&reparsed),
        is_explain: matches!(reparsed, Statement::Explain { .. }),
        sql: reparsed.to_string(),
        translated_literals: outcome.injected.len(),
        injected: outcome.injected,
        warnings: outcome.warnings,
    })
}

fn parse_single(sql: &str) -> Result<Statement, ToolError> {
    let mut statements = Parser::parse_sql(&PostgreSqlDialect {}, sql).map_err(|error| {
        ToolError::new(ErrorCategory::RefusedReadOnly, "parse_error", error.to_string()).with_hint(
            "the statement could not be parsed; write a single standard SELECT, WITH or EXPLAIN statement",
        )
    })?;
    match statements.len() {
        1 => Ok(statements.remove(0)),
        0 => Err(ToolError::new(
            ErrorCategory::RefusedReadOnly,
            "empty_statement",
            "no SQL statement was given",
        )),
        _ => Err(ToolError::new(
            ErrorCategory::RefusedReadOnly,
            "multi_statement",
            "only one statement per call is accepted",
        )
        .with_hint("send each statement in its own query call")),
    }
}
