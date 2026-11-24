mod extended_json;
mod limits;
mod operators;
mod stages;

use serde_json::{Map, Value, json};

use super::mongo_filter_walker::{LiteralRole, LiteralSite, Segment, lookup_texts, translate};
use crate::contexts::querying::domain::constants::mongo_policy::{
    ALLOWLIST_HINT, MAX_ECHOED_NAME_CHARS, QUERY_COMMENT,
};
use crate::contexts::querying::domain::errors::Refusal;
use crate::contexts::querying::domain::types::{
    AggregateRequest, ClampWarning, FindRequest, GuardedAggregate, GuardedFind, GuardedMongo,
    MongoCommand, MongoExecution, MongoReadPreference, QueryLimits, Translations,
};

pub struct InspectedMongo {
    database: Option<String>,
    collection: String,
    root: Map<String, Value>,
    find: Option<FindBounds>,
    sites: Vec<LiteralSite>,
}

struct FindBounds {
    skip: Option<u64>,
    limit: Option<u64>,
}

pub fn check_names(database: &str, collection: Option<&str>) -> Result<(), Refusal> {
    limits::check_namespace(Some(database), collection.unwrap_or("documents"))
}

pub fn inspect_find(request: &FindRequest) -> Result<InspectedMongo, Refusal> {
    limits::check_namespace(request.database.as_deref(), &request.collection)?;
    let mut root = Map::new();
    root.insert(
        "filter".into(),
        request
            .filter
            .clone()
            .unwrap_or_else(|| Value::Object(Map::new())),
    );
    for (name, part) in [
        ("projection", &request.projection),
        ("sort", &request.sort),
        ("hint", &request.hint),
        ("collation", &request.collation),
    ] {
        if let Some(value) = part {
            root.insert(name.into(), value.clone());
        }
    }
    limits::check_request(&root)?;
    let mut walker = Walker::default();
    walker.find(&root)?;
    Ok(InspectedMongo {
        database: request.database.clone(),
        collection: request.collection.clone(),
        root,
        find: Some(FindBounds {
            skip: request.skip,
            limit: request.limit,
        }),
        sites: walker.sites,
    })
}

pub fn inspect_aggregate(request: &AggregateRequest) -> Result<InspectedMongo, Refusal> {
    limits::check_namespace(request.database.as_deref(), &request.collection)?;
    let mut root = Map::new();
    root.insert("pipeline".into(), Value::Array(request.pipeline.clone()));
    if let Some(value) = &request.let_vars {
        root.insert("let".into(), value.clone());
    }
    limits::check_request(&root)?;
    let mut walker = Walker::default();
    walker.aggregate(&root)?;
    Ok(InspectedMongo {
        database: request.database.clone(),
        collection: request.collection.clone(),
        root,
        find: None,
        sites: walker.sites,
    })
}

impl InspectedMongo {
    pub fn literals(&self) -> Vec<String> {
        lookup_texts(&self.sites)
    }

    pub fn finish(mut self, translations: &Translations, limits: &QueryLimits) -> GuardedMongo {
        let outcome = translate(&mut self.root, &self.sites, translations);
        let max_documents = limits.max_rows.saturating_add(1);
        let cap = u64::try_from(max_documents).unwrap_or(u64::MAX);
        let mut clamps = Vec::new();
        let command = match self.find {
            Some(bounds) => {
                let limit = match bounds.limit {
                    Some(requested) if requested > cap => {
                        clamps.push(ClampWarning {
                            code: "limit_clamped",
                            requested,
                            applied: cap - 1,
                        });
                        cap
                    }
                    Some(requested) if requested > 0 => requested,
                    _ => cap,
                };
                MongoCommand::Find(Box::new(GuardedFind {
                    filter: self
                        .root
                        .remove("filter")
                        .unwrap_or_else(|| Value::Object(Map::new())),
                    projection: self.root.remove("projection"),
                    sort: self.root.remove("sort"),
                    skip: bounds.skip,
                    limit,
                    hint: self.root.remove("hint"),
                    collation: self.root.remove("collation"),
                }))
            }
            None => {
                let mut pipeline = match self.root.remove("pipeline") {
                    Some(Value::Array(stages)) => stages,
                    _ => Vec::new(),
                };
                pipeline.push(json!({ "$limit": cap }));
                MongoCommand::Aggregate(GuardedAggregate {
                    pipeline,
                    let_vars: self.root.remove("let"),
                })
            }
        };
        GuardedMongo {
            database: self.database,
            collection: self.collection,
            command,
            execution: MongoExecution {
                max_time_ms: limits.timeout_ms,
                allow_disk_use: false,
                read_preference: MongoReadPreference::SecondaryPreferred,
                batch_size: u32::try_from(max_documents).unwrap_or(u32::MAX),
                max_documents,
                comment: QUERY_COMMENT,
            },
            translated_literals: outcome.injected.len(),
            injected: outcome.injected,
            warnings: outcome.warnings,
            clamps,
        }
    }
}

#[derive(Default)]
struct Walker {
    path: Vec<Segment>,
    sites: Vec<LiteralSite>,
    in_expr: bool,
    stages: usize,
}

impl Walker {
    fn enter<T>(
        &mut self,
        segment: Segment,
        visit: impl FnOnce(&mut Self) -> Result<T, Refusal>,
    ) -> Result<T, Refusal> {
        self.path.push(segment);
        let result = visit(self);
        self.path.pop();
        result
    }

    fn enter_key<T>(
        &mut self,
        key: &str,
        visit: impl FnOnce(&mut Self) -> Result<T, Refusal>,
    ) -> Result<T, Refusal> {
        self.enter(Segment::Key(key.to_string()), |walker| {
            if key.contains('\0') {
                return Err(walker.refuse(
                    "mongo_invalid_field_name",
                    "a field name contains a NUL character",
                ));
            }
            visit(walker)
        })
    }

    fn enter_index<T>(
        &mut self,
        index: usize,
        visit: impl FnOnce(&mut Self) -> Result<T, Refusal>,
    ) -> Result<T, Refusal> {
        self.enter(Segment::Index(index), visit)
    }

    fn record(&mut self, value: &Value, role: LiteralRole) {
        let (text, numeric) = match value {
            Value::String(text) => (text.clone(), false),
            Value::Number(number) if number.is_i64() || number.is_u64() => {
                (number.to_string(), true)
            }
            _ => return,
        };
        self.sites.push(LiteralSite {
            pointer: self.path.clone(),
            role,
            text,
            numeric,
        });
    }

    fn compared_role(&self) -> LiteralRole {
        if self.in_expr {
            LiteralRole::ComparedInExpression
        } else {
            LiteralRole::Compared
        }
    }

    fn position(&self) -> String {
        let mut rendered = String::new();
        for segment in &self.path {
            match segment {
                Segment::Key(key) => {
                    if !rendered.is_empty() {
                        rendered.push('.');
                    }
                    rendered.push_str(&echoable(key));
                }
                Segment::Index(index) => rendered.push_str(&format!("[{index}]")),
            }
        }
        match self.path.as_slice() {
            [Segment::Key(root), Segment::Index(stage), ..] if root == "pipeline" => {
                format!("stage {}, {rendered}", stage + 1)
            }
            _ => rendered,
        }
    }

    fn refuse(&self, code: &str, what: &str) -> Refusal {
        Refusal::new(code, format!("{what} ({})", self.position()))
    }

    fn not_allowed(&self, code: &str, name: &str, context: &str) -> Refusal {
        self.refuse(
            code,
            &format!("{} is not an allowed {context}", echoable(name)),
        )
        .with_hint(ALLOWLIST_HINT)
    }

    fn invalid(&self, what: &str) -> Refusal {
        self.refuse("mongo_invalid_argument", what)
    }
}

fn echoable(name: &str) -> String {
    let body = name.trim_start_matches('$');
    let readable = !body.is_empty()
        && name.chars().count() <= MAX_ECHOED_NAME_CHARS
        && name.len() - body.len() <= 2
        && body
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | '.' | '-'));
    if readable {
        name.to_string()
    } else {
        "<name>".to_string()
    }
}
