use sqlparser::ast::{Query, Select, SelectItem, SetExpr, TableAlias, TableFactor, TableWithJoins};

use super::expression::is_clear;
use super::names::{is_wildcard, name_parts, normalized, output_name};
use super::relation_scope::{RelationScope, ScopeRelation};
use crate::contexts::querying::domain::types::{CatalogColumn, DerivedColumn, RelationCatalog};

type Ctes = Vec<(String, Option<Vec<DerivedColumn>>)>;

pub struct ProjectionLineage<'a> {
    pub catalog: &'a RelationCatalog,
    pub is_clear_source: &'a dyn Fn(&CatalogColumn) -> bool,
}

impl<'a> ProjectionLineage<'a> {
    pub fn columns(&self, query: &Query) -> Option<Vec<DerivedColumn>> {
        self.query_columns(query, &Vec::new())
    }

    fn query_columns(&self, query: &Query, outer: &Ctes) -> Option<Vec<DerivedColumn>> {
        let mut ctes = outer.clone();
        if let Some(with) = &query.with {
            if with.recursive {
                return None;
            }
            for cte in &with.cte_tables {
                let columns = self
                    .query_columns(&cte.query, &ctes)
                    .and_then(|columns| renamed(columns, Some(&cte.alias)));
                ctes.push((normalized(&cte.alias.name), columns));
            }
        }
        self.set_columns(&query.body, &ctes)
    }

    fn set_columns(&self, body: &SetExpr, ctes: &Ctes) -> Option<Vec<DerivedColumn>> {
        match body {
            SetExpr::Select(select) => self.select_columns(select, ctes),
            SetExpr::Query(query) => self.query_columns(query, ctes),
            SetExpr::SetOperation { left, right, .. } => {
                let left = self.set_columns(left, ctes)?;
                let right = self.set_columns(right, ctes)?;
                (left.len() == right.len()).then(|| {
                    left.into_iter()
                        .zip(right)
                        .map(|(left, right)| DerivedColumn {
                            name: left.name,
                            clear: left.clear && right.clear,
                        })
                        .collect()
                })
            }
            SetExpr::Values(values) => {
                let scope = RelationScope::new(Vec::new(), self.is_clear_source);
                let width = values.rows.first()?.content.len();
                if values.rows.iter().any(|row| row.content.len() != width) {
                    return None;
                }
                Some(
                    (0..width)
                        .map(|index| DerivedColumn {
                            name: Some(format!("column{}", index + 1)),
                            clear: values
                                .rows
                                .iter()
                                .all(|row| is_clear(&row.content[index], &scope)),
                        })
                        .collect(),
                )
            }
            _ => None,
        }
    }

    fn select_columns(&self, select: &Select, ctes: &Ctes) -> Option<Vec<DerivedColumn>> {
        let scope = RelationScope::new(self.relations(&select.from, ctes), self.is_clear_source);
        select
            .projection
            .iter()
            .map(|item| match item {
                SelectItem::UnnamedExpr(expr) if !is_wildcard(expr) => Some(DerivedColumn {
                    name: output_name(expr),
                    clear: is_clear(expr, &scope),
                }),
                SelectItem::ExprWithAlias { expr, alias } if !is_wildcard(expr) => {
                    Some(DerivedColumn {
                        name: Some(normalized(alias)),
                        clear: is_clear(expr, &scope),
                    })
                }
                _ => None,
            })
            .collect()
    }

    fn relations(&self, from: &[TableWithJoins], ctes: &Ctes) -> Vec<ScopeRelation<'a>> {
        let mut relations = Vec::new();
        for table in from {
            self.add_joined(table, ctes, &mut relations);
        }
        relations
    }

    fn add_joined(
        &self,
        table: &TableWithJoins,
        ctes: &Ctes,
        relations: &mut Vec<ScopeRelation<'a>>,
    ) {
        self.add_relation(&table.relation, ctes, relations);
        for join in &table.joins {
            self.add_relation(&join.relation, ctes, relations);
        }
    }

    fn add_relation(
        &self,
        factor: &TableFactor,
        ctes: &Ctes,
        relations: &mut Vec<ScopeRelation<'a>>,
    ) {
        if let TableFactor::NestedJoin {
            table_with_joins,
            alias: None,
        } = factor
        {
            self.add_joined(table_with_joins, ctes, relations);
        } else {
            relations.push(self.relation(factor, ctes));
        }
    }

    fn relation(&self, factor: &TableFactor, ctes: &Ctes) -> ScopeRelation<'a> {
        match factor {
            TableFactor::Table {
                name,
                alias,
                args: None,
                ..
            } => {
                let Some(parts) = name_parts(name) else {
                    return ScopeRelation::Opaque;
                };
                let cte = match parts.as_slice() {
                    [single] => ctes.iter().rev().find(|(cte, _)| cte == single),
                    _ => None,
                };
                let visible = alias
                    .as_ref()
                    .map(|alias| normalized(&alias.name))
                    .or_else(|| parts.last().cloned());
                match cte {
                    Some((_, columns)) => {
                        match columns
                            .clone()
                            .and_then(|columns| renamed(columns, alias.as_ref()))
                        {
                            Some(columns) => ScopeRelation::Derived {
                                name: visible,
                                columns,
                            },
                            None => ScopeRelation::Opaque,
                        }
                    }
                    None => match (
                        self.catalog.columns(&name.to_string()),
                        visible,
                        alias.as_ref().is_none_or(|alias| alias.columns.is_empty()),
                    ) {
                        (Some(columns), Some(name), true) => {
                            ScopeRelation::Catalog { name, columns }
                        }
                        _ => ScopeRelation::Opaque,
                    },
                }
            }
            TableFactor::Derived {
                lateral: false,
                subquery,
                alias,
                ..
            } => match self
                .query_columns(subquery, ctes)
                .and_then(|columns| renamed(columns, alias.as_ref()))
            {
                Some(columns) => ScopeRelation::Derived {
                    name: alias.as_ref().map(|alias| normalized(&alias.name)),
                    columns,
                },
                None => ScopeRelation::Opaque,
            },
            _ => ScopeRelation::Opaque,
        }
    }
}

fn renamed(
    mut columns: Vec<DerivedColumn>,
    alias: Option<&TableAlias>,
) -> Option<Vec<DerivedColumn>> {
    let Some(alias) = alias else {
        return Some(columns);
    };
    if alias.columns.len() > columns.len() {
        return None;
    }
    for (column, renamed) in columns.iter_mut().zip(&alias.columns) {
        column.name = Some(normalized(&renamed.name));
    }
    Some(columns)
}
