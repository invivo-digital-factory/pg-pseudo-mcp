use sqlparser::ast::Statement;

use super::lineage::{derived_lineage, names_asked_by_lineage};
use super::literal_collector::collect_literals;
use super::literal_rewriter::rewrite_literals;
use super::statement_parser::parse_single;
use super::statement_validator::validate;
use crate::contexts::querying::domain::errors::Refusal;
use crate::contexts::querying::domain::types::{
    Guarded, Lineage, PersonalNames, Translations, Warning,
};

pub struct Inspected {
    statement: Statement,
    literals: Vec<String>,
}

pub struct Rewritten {
    statement: Statement,
    sql: String,
    injected: Vec<(String, String)>,
    warnings: Vec<Warning>,
}

pub fn inspect(sql: &str) -> Result<Inspected, Refusal> {
    let statement = parse_single(sql)?;
    validate(&statement)?;
    let literals = collect_literals(&statement);
    Ok(Inspected {
        statement,
        literals,
    })
}

impl Inspected {
    pub fn literals(&self) -> &[String] {
        &self.literals
    }

    pub fn rewrite(mut self, translations: &Translations) -> Result<Rewritten, Refusal> {
        let outcome = rewrite_literals(&mut self.statement, translations);
        let rewritten = self.statement.to_string();
        let reparsed = parse_single(&rewritten).map_err(|_| {
            Refusal::new(
                "rewrite_failed",
                "the statement could not be safely re-serialized after translating pseudonyms",
            )
            .with_hint("rewrite the query with simpler syntax")
        })?;
        validate(&reparsed)?;
        Ok(Rewritten {
            sql: reparsed.to_string(),
            statement: reparsed,
            injected: outcome.injected,
            warnings: outcome.warnings,
        })
    }
}

impl Rewritten {
    pub fn lineage_names(&self) -> Vec<String> {
        names_asked_by_lineage(&self.statement)
    }

    pub fn finish(self, personal: &PersonalNames) -> Guarded {
        let derived_lineage = derived_lineage(&self.statement, personal);
        self.guarded(derived_lineage)
    }

    pub fn finish_without_lineage(self) -> Guarded {
        self.guarded(None)
    }

    fn guarded(self, derived_lineage: Option<Vec<Lineage>>) -> Guarded {
        Guarded {
            derived_lineage,
            is_explain: matches!(self.statement, Statement::Explain { .. }),
            sql: self.sql,
            translated_literals: self.injected.len(),
            injected: self.injected,
            warnings: self.warnings,
        }
    }
}
