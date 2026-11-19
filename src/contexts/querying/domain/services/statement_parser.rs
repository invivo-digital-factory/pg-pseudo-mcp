use sqlparser::ast::Statement;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use crate::contexts::querying::domain::errors::Refusal;

pub fn parse_single(sql: &str) -> Result<Statement, Refusal> {
    let mut statements = Parser::parse_sql(&PostgreSqlDialect {}, sql).map_err(|error| {
        Refusal::new("parse_error", error.to_string()).with_hint(
            "the statement could not be parsed; write a single standard SELECT, WITH or EXPLAIN statement",
        )
    })?;
    match statements.len() {
        1 => Ok(statements.remove(0)),
        0 => Err(Refusal::new(
            "empty_statement",
            "no SQL statement was given",
        )),
        _ => Err(
            Refusal::new("multi_statement", "only one statement per call is accepted")
                .with_hint("send each statement in its own query call"),
        ),
    }
}
