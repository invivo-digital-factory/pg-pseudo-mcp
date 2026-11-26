mod expressions;
mod shape;
mod stages;

use shape::Shape;
use stages::{Pipeline, project};

use crate::contexts::querying::domain::constants::clear_lineage::CLEAR_MONGO_OPERATORS;
use crate::contexts::querying::domain::types::{
    ColumnPolicy, FieldLineage, FieldSource, GuardedMongo, LineageClearance, MongoCommand,
    PolicyDecision,
};

pub struct DocumentLineage {
    root: Shape,
}

impl DocumentLineage {
    pub fn of(&self, path: &str) -> FieldLineage {
        if path.is_empty() {
            return self.root.lineage();
        }
        let segments: Vec<String> = path.split('.').map(str::to_string).collect();
        self.root.at(&segments).lineage()
    }

    pub fn root_is_computed(&self) -> bool {
        matches!(&self.root, Shape::Value(lineage) if !matches!(lineage, FieldLineage::Source(_)))
    }

    pub fn own_of_segments(&self, segments: &[String]) -> FieldLineage {
        self.root.at(segments).own_lineage()
    }
}

pub fn document_lineage(guarded: &GuardedMongo, database: &str) -> DocumentLineage {
    let start = Shape::collection(database, &guarded.collection);
    let root = match &guarded.command {
        MongoCommand::Find(find) => match &find.projection {
            Some(projection) => project(&start, projection),
            None => start,
        },
        MongoCommand::Aggregate(aggregate) => Pipeline { database }.run(start, &aggregate.pipeline),
    };
    DocumentLineage { root }
}

pub fn clearance(lineage: &FieldLineage, policy: &ColumnPolicy) -> LineageClearance {
    match lineage {
        FieldLineage::Source(source) => LineageClearance::Source(source.clone()),
        FieldLineage::Derived { inputs, operators } => LineageClearance::Derived {
            proven_clear: operators
                .iter()
                .all(|name| CLEAR_MONGO_OPERATORS.contains(&name.as_str()))
                && inputs.iter().all(|input| is_clear_input(input, policy)),
        },
        FieldLineage::Unknown => LineageClearance::Derived {
            proven_clear: false,
        },
    }
}

fn is_clear_input(input: &FieldSource, policy: &ColumnPolicy) -> bool {
    policy.decide_field(input) == Some(&PolicyDecision::Clear)
        && !policy.has_personal_field_below(input)
}
