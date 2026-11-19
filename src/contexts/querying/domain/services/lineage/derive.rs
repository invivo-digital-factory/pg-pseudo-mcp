use sqlparser::ast::Statement;

use super::personal_name::{NameJudge, NameRecorder};
use super::scope::Scope;
use super::shape::Shape;
use super::taint::tainted_names;
use crate::contexts::querying::domain::types::Lineage;

pub fn derived_lineage(statement: &Statement, judge: &dyn NameJudge) -> Option<Vec<Lineage>> {
    let Statement::Query(query) = statement else {
        return None;
    };
    let tainted = tainted_names(statement, judge);
    let shape = Shape::of_query(
        query,
        Scope {
            tainted: &tainted,
            judge,
        },
    );
    let mut lineage: Vec<Lineage> = shape
        .items
        .iter()
        .map(|personal| {
            if *personal {
                Lineage::Personal
            } else {
                Lineage::Clean
            }
        })
        .collect();
    if shape.rest {
        lineage.push(Lineage::Unknown);
    }
    Some(lineage)
}

pub fn names_asked_by_lineage(statement: &Statement) -> Vec<String> {
    let recorder = NameRecorder::default();
    let _ = derived_lineage(statement, &recorder);
    recorder.into_names()
}
