use std::collections::BTreeMap;

use crate::contexts::querying::domain::types::{FieldLineage, FieldSource};

#[derive(Debug, Clone)]
pub enum Shape {
    Value(FieldLineage),
    Document {
        fields: BTreeMap<String, Shape>,
        rest: Option<FieldLineage>,
    },
}

impl Shape {
    pub fn collection(database: &str, collection: &str) -> Self {
        Self::Document {
            fields: BTreeMap::new(),
            rest: Some(FieldLineage::Source(FieldSource {
                database: database.to_string(),
                collection: collection.to_string(),
                path: String::new(),
            })),
        }
    }

    pub fn built() -> Self {
        Self::Document {
            fields: BTreeMap::new(),
            rest: None,
        }
    }

    pub fn constant() -> Self {
        Self::counted(&[])
    }

    pub fn counted(operators: &[&str]) -> Self {
        Self::Value(FieldLineage::Derived {
            inputs: Vec::new(),
            operators: operators.iter().map(|name| name.to_string()).collect(),
        })
    }

    pub fn unknown() -> Self {
        Self::Value(FieldLineage::Unknown)
    }

    pub fn at(&self, path: &[String]) -> Shape {
        let Some((head, tail)) = path.split_first() else {
            return self.clone();
        };
        match self {
            Self::Value(lineage) => Self::Value(below(lineage, path)),
            Self::Document { fields, rest } => match (fields.get(head), rest) {
                (Some(shape), _) => shape.at(tail),
                (None, Some(lineage)) => Self::Value(below(lineage, path)),
                (None, None) => Self::unknown(),
            },
        }
    }

    pub fn set(&mut self, path: &[String], shape: Shape) {
        let Some((head, tail)) = path.split_first() else {
            *self = shape;
            return;
        };
        if let Self::Value(lineage) = self {
            *self = Self::Document {
                fields: BTreeMap::new(),
                rest: Some(lineage.clone()),
            };
        }
        if let Self::Document { fields, rest } = self {
            fields
                .entry(head.clone())
                .or_insert_with(|| match rest {
                    Some(lineage) => Self::Value(below(lineage, std::slice::from_ref(head))),
                    None => Self::built(),
                })
                .set(tail, shape);
        }
    }

    pub fn lineage(&self) -> FieldLineage {
        match self {
            Self::Value(lineage) => lineage.clone(),
            Self::Document { fields, rest } if fields.is_empty() => match rest {
                Some(lineage) => lineage.clone(),
                None => FieldLineage::Derived {
                    inputs: Vec::new(),
                    operators: Vec::new(),
                },
            },
            Self::Document { fields, rest: None } => {
                traversed(fields).map_or_else(|| self.derived(), FieldLineage::Source)
            }
            Self::Document { .. } => self.derived(),
        }
    }

    pub fn own_lineage(&self) -> FieldLineage {
        match self {
            Self::Document {
                rest: Some(FieldLineage::Source(source)),
                ..
            } => FieldLineage::Source(source.clone()),
            _ => self.lineage(),
        }
    }

    fn derived(&self) -> FieldLineage {
        let mut derivation = Derivation::default();
        self.contribute(&mut derivation);
        derivation.finish()
    }

    pub fn contribute(&self, derivation: &mut Derivation) {
        match self {
            Self::Value(lineage) => derivation.absorb(lineage),
            Self::Document { fields, rest } => {
                for shape in fields.values() {
                    shape.contribute(derivation);
                }
                if let Some(lineage) = rest {
                    derivation.absorb(lineage);
                }
            }
        }
    }
}

#[derive(Default)]
pub struct Derivation {
    inputs: Vec<FieldSource>,
    operators: Vec<String>,
    unknown: bool,
}

impl Derivation {
    pub fn absorb(&mut self, lineage: &FieldLineage) {
        match lineage {
            FieldLineage::Source(source) => push_new(&mut self.inputs, source),
            FieldLineage::Derived { inputs, operators } => {
                for input in inputs {
                    push_new(&mut self.inputs, input);
                }
                for operator in operators {
                    push_new(&mut self.operators, operator);
                }
            }
            FieldLineage::Unknown => self.unknown = true,
        }
    }

    pub fn operator(&mut self, name: &str) {
        push_new(&mut self.operators, &name.to_string());
    }

    pub fn finish(self) -> FieldLineage {
        if self.unknown {
            FieldLineage::Unknown
        } else {
            FieldLineage::Derived {
                inputs: self.inputs,
                operators: self.operators,
            }
        }
    }
}

fn traversed(fields: &BTreeMap<String, Shape>) -> Option<FieldSource> {
    let mut parents = fields.iter().map(|(key, shape)| match shape.lineage() {
        FieldLineage::Source(source) => parent(&source, key),
        _ => None,
    });
    let first = parents.next()??;
    parents
        .all(|other| other.as_ref() == Some(&first))
        .then_some(first)
}

fn parent(source: &FieldSource, key: &str) -> Option<FieldSource> {
    let path = if source.path == key {
        String::new()
    } else {
        source
            .path
            .strip_suffix(key)?
            .strip_suffix('.')?
            .to_string()
    };
    Some(FieldSource {
        path,
        ..source.clone()
    })
}

fn push_new<T: Clone + PartialEq>(items: &mut Vec<T>, item: &T) {
    if !items.contains(item) {
        items.push(item.clone());
    }
}

fn below(lineage: &FieldLineage, path: &[String]) -> FieldLineage {
    match lineage {
        FieldLineage::Source(source) => FieldLineage::Source(extended(source, path)),
        FieldLineage::Derived { inputs, operators } => FieldLineage::Derived {
            inputs: inputs.iter().map(|input| extended(input, path)).collect(),
            operators: operators.clone(),
        },
        FieldLineage::Unknown => FieldLineage::Unknown,
    }
}

fn extended(source: &FieldSource, path: &[String]) -> FieldSource {
    let tail = path.join(".");
    FieldSource {
        path: if source.path.is_empty() {
            tail
        } else {
            format!("{}.{tail}", source.path)
        },
        ..source.clone()
    }
}
