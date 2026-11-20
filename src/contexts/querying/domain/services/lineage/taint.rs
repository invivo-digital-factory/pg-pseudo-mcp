use std::collections::HashSet;
use std::ops::ControlFlow;

use sqlparser::ast::{
    Query, Select, SelectItem, Statement, TableAlias, TableFactor, Visit, Visitor,
};

use super::implicit_names::implicit_names;
use super::personal_name::NameJudge;
use super::references::references;
use super::scope::Scope;
use super::shape::Shape;
use super::sql_names::{factor_alias, function_name, last_part};

pub fn tainted_names(statement: &Statement, judge: &dyn NameJudge) -> HashSet<String> {
    let mut relations = Relations::default();
    let _ = statement.visit(&mut relations);
    let mut collector = Collector {
        tainted: relations.names,
        judge,
        changed: false,
    };
    loop {
        collector.changed = false;
        let _ = statement.visit(&mut collector);
        if !collector.changed {
            return collector.tainted;
        }
    }
}

#[derive(Default)]
struct Relations {
    names: HashSet<String>,
}

impl Visitor for Relations {
    type Break = ();

    fn pre_visit_table_factor(&mut self, factor: &TableFactor) -> ControlFlow<()> {
        if let TableFactor::Table {
            name,
            alias,
            args: None,
            ..
        } = factor
        {
            let relation = last_part(name).map(|part| part.value.to_lowercase());
            let alias = alias.as_ref().map(|alias| alias.name.value.to_lowercase());
            self.names.extend(relation.into_iter().chain(alias));
        }
        ControlFlow::Continue(())
    }
}

struct Collector<'a> {
    tainted: HashSet<String>,
    judge: &'a dyn NameJudge,
    changed: bool,
}

impl Collector<'_> {
    fn scope(&self) -> Scope<'_> {
        Scope {
            tainted: &self.tainted,
            judge: self.judge,
        }
    }

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

impl Visitor for Collector<'_> {
    type Break = ();

    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<()> {
        if let Some(with) = &query.with {
            for cte in &with.cte_tables {
                let shape = Shape::of_query(&cte.query, self.scope());
                self.taint_alias(&cte.alias, &shape);
            }
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_select(&mut self, select: &Select) -> ControlFlow<()> {
        let mut names = Vec::new();
        for item in &select.projection {
            match item {
                SelectItem::ExprWithAlias { expr, alias } if references(expr, self.scope()) => {
                    names.push(alias.value.clone());
                }
                SelectItem::ExprWithAliases { expr, aliases } if references(expr, self.scope()) => {
                    names.extend(aliases.iter().map(|alias| alias.value.clone()));
                }
                SelectItem::UnnamedExpr(expr) if references(expr, self.scope()) => {
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
                let shape = Shape::of_query(subquery, self.scope());
                self.taint_alias(alias, &shape);
            }
            TableFactor::Derived { .. } => {}
            TableFactor::Table {
                name, alias, args, ..
            } => {
                let from_function = args
                    .as_ref()
                    .is_some_and(|args| references(args, self.scope()));
                let from_relation =
                    last_part(name).is_some_and(|part| self.scope().is_tainted(part));
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
                if references(other, self.scope()) {
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
