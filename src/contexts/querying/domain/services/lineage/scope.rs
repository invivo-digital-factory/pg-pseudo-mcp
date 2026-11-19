use std::collections::HashSet;

use sqlparser::ast::Ident;

use super::personal_name::{NameJudge, is_personal_name};

#[derive(Clone, Copy)]
pub struct Scope<'a> {
    pub tainted: &'a HashSet<String>,
    pub judge: &'a dyn NameJudge,
}

impl Scope<'_> {
    pub fn is_tainted(&self, ident: &Ident) -> bool {
        is_personal_name(&ident.value, self.judge)
            || self.tainted.contains(&ident.value.to_lowercase())
    }
}
