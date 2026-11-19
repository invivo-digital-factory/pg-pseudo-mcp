use sqlparser::ast::{Expr, Query, SelectItem, SetExpr};

use super::references::references;
use super::scope::Scope;

#[derive(Debug, Default)]
pub struct Shape {
    pub items: Vec<bool>,
    pub rest: bool,
}

impl Shape {
    pub fn all() -> Self {
        Self {
            items: Vec::new(),
            rest: true,
        }
    }

    pub fn any(&self) -> bool {
        self.rest || self.items.iter().any(|item| *item)
    }

    pub fn at(&self, index: usize) -> bool {
        self.items.get(index).copied().unwrap_or(self.rest)
    }

    pub fn of_query(query: &Query, scope: Scope<'_>) -> Self {
        Self::of_set_expr(&query.body, scope)
    }

    fn of_set_expr(body: &SetExpr, scope: Scope<'_>) -> Self {
        match body {
            SetExpr::Select(select) => Self::of_projection(&select.projection, scope),
            SetExpr::Query(query) => Self::of_query(query, scope),
            SetExpr::SetOperation { left, right, .. } => {
                Self::of_set_expr(left, scope).merge(Self::of_set_expr(right, scope))
            }
            SetExpr::Values(values) => values
                .rows
                .iter()
                .map(|row| Self {
                    items: row
                        .content
                        .iter()
                        .map(|expr| references(expr, scope))
                        .collect(),
                    rest: false,
                })
                .reduce(Self::merge)
                .unwrap_or_default(),
            _ => Self::all(),
        }
    }

    fn of_projection(projection: &[SelectItem], scope: Scope<'_>) -> Self {
        let mut items = Vec::with_capacity(projection.len());
        for item in projection {
            match item {
                SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. }
                    if !is_wildcard(expr) =>
                {
                    items.push(references(expr, scope));
                }
                SelectItem::ExprWithAliases { expr, aliases } => {
                    let flag = references(expr, scope);
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
