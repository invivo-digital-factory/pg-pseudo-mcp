use std::collections::{BTreeSet, HashSet};
use std::ops::ControlFlow;

use sqlparser::ast::{Query, Statement, TableFactor, Visit, Visitor};

use super::names::{name_parts, normalized};

pub fn catalog_relations(statement: &Statement) -> Vec<String> {
    let mut ctes = CteNames::default();
    let _ = statement.visit(&mut ctes);
    let mut tables = Tables {
        ctes: ctes.names,
        found: BTreeSet::new(),
    };
    let _ = statement.visit(&mut tables);
    tables.found.into_iter().collect()
}

#[derive(Default)]
struct CteNames {
    names: HashSet<String>,
}

impl Visitor for CteNames {
    type Break = ();

    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<()> {
        if let Some(with) = &query.with {
            self.names.extend(
                with.cte_tables
                    .iter()
                    .map(|cte| normalized(&cte.alias.name)),
            );
        }
        ControlFlow::Continue(())
    }
}

struct Tables {
    ctes: HashSet<String>,
    found: BTreeSet<String>,
}

impl Visitor for Tables {
    type Break = ();

    fn pre_visit_table_factor(&mut self, factor: &TableFactor) -> ControlFlow<()> {
        if let TableFactor::Table {
            name, args: None, ..
        } = factor
            && let Some(parts) = name_parts(name)
            && !matches!(parts.as_slice(), [single] if self.ctes.contains(single))
        {
            self.found.insert(name.to_string());
        }
        ControlFlow::Continue(())
    }
}
