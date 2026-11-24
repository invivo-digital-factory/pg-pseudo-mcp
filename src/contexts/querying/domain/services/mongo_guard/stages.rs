use serde_json::{Map, Value};

use super::Walker;
use super::limits::{NameProblem, collection_problem, system_refusal};
use crate::contexts::querying::domain::constants::mongo_policy::{
    BUCKET_ARGUMENTS, BUCKET_AUTO_ARGUMENTS, COLLATION_ARGUMENTS, CROSS_DATABASE_HINT,
    DENSIFY_ARGUMENTS, EXPRESSION_OPERATORS, FILL_ARGUMENTS, FILL_OUTPUT_ARGUMENTS,
    FIRST_STAGE_ONLY, GEO_NEAR_ARGUMENTS, GRAPH_LOOKUP_ARGUMENTS, LOOKUP_ARGUMENTS,
    MAX_PIPELINE_STAGES, REPLACE_ROOT_ARGUMENTS, SAMPLE_ARGUMENTS, SET_WINDOW_FIELDS_ARGUMENTS,
    STAGES, UNION_WITH_ARGUMENTS, UNWIND_ARGUMENTS, WINDOW_ARGUMENT,
};
use crate::contexts::querying::domain::errors::Refusal;

impl Walker {
    pub(super) fn find(&mut self, root: &Map<String, Value>) -> Result<(), Refusal> {
        for (part, value) in root {
            self.enter_key(part, |walker| match part.as_str() {
                "filter" => walker.filter(value),
                "projection" => walker.projection(value),
                "sort" => walker.sort(value),
                "hint" => walker.hint(value),
                _ => walker.arguments(
                    value,
                    "collation",
                    COLLATION_ARGUMENTS,
                    |walker, _, item| walker.literal(item, None),
                ),
            })?;
        }
        Ok(())
    }

    pub(super) fn aggregate(&mut self, root: &Map<String, Value>) -> Result<(), Refusal> {
        for (part, value) in root {
            self.enter_key(part, |walker| match (part.as_str(), value) {
                ("pipeline", Value::Array(stages)) => walker.pipeline(stages, true),
                _ => walker.fields(value, "let", Self::expression),
            })?;
        }
        Ok(())
    }

    fn pipeline(&mut self, stages: &[Value], top_level: bool) -> Result<(), Refusal> {
        if stages.len() > MAX_PIPELINE_STAGES {
            return Err(self.refuse(
                "mongo_pipeline_too_long",
                &format!("a pipeline holds more than {MAX_PIPELINE_STAGES} stages"),
            ));
        }
        self.stages += stages.len();
        if self.stages > MAX_PIPELINE_STAGES {
            return Err(self.refuse(
                "mongo_pipeline_too_long",
                &format!(
                    "the pipeline and its sub-pipelines hold more than {MAX_PIPELINE_STAGES} stages in all"
                ),
            ));
        }
        for (index, stage) in stages.iter().enumerate() {
            self.enter_index(index, |walker| walker.stage(stage, top_level && index == 0))?;
        }
        Ok(())
    }

    fn sub_pipeline(&mut self, value: &Value) -> Result<(), Refusal> {
        let Value::Array(stages) = value else {
            return Err(self.invalid("a sub-pipeline must be an array of stages"));
        };
        let outer = self.in_expr;
        self.in_expr = false;
        let result = self.pipeline(stages, false);
        self.in_expr = outer;
        result
    }

    fn stage(&mut self, stage: &Value, first: bool) -> Result<(), Refusal> {
        let Value::Object(map) = stage else {
            return Err(self.invalid("a pipeline stage must be an object"));
        };
        for name in map.keys() {
            self.enter_key(name, |walker| {
                if STAGES.contains(&name.as_str()) {
                    Ok(())
                } else {
                    Err(walker.not_allowed("mongo_stage_not_allowed", name, "aggregation stage"))
                }
            })?;
        }
        let mut entries = map.iter();
        let (Some((name, argument)), None) = (entries.next(), entries.next()) else {
            return Err(self.invalid("a pipeline stage must hold exactly one stage operator"));
        };
        self.enter_key(name, |walker| {
            if FIRST_STAGE_ONLY.contains(&name.as_str()) && !first {
                return Err(walker.refuse(
                    "mongo_stage_position",
                    &format!("{name} is only allowed as the first stage of the top-level pipeline"),
                ));
            }
            walker.stage_argument(name, argument)
        })
    }

    fn stage_argument(&mut self, name: &str, argument: &Value) -> Result<(), Refusal> {
        match name {
            "$match" => self.filter(argument),
            "$project" => self.projection(argument),
            "$addFields" | "$set" | "$group" => self.fields(argument, name, Self::expression),
            "$unset" => self.unset(argument),
            "$sort" => self.sort(argument),
            "$limit" => self.integer(argument, name, 1),
            "$skip" => self.integer(argument, name, 0),
            "$count" => match argument {
                Value::String(_) => Ok(()),
                _ => Err(self.invalid("$count takes the name of its output field")),
            },
            "$unwind" => match argument {
                Value::String(_) => Ok(()),
                _ => self.arguments(argument, name, UNWIND_ARGUMENTS, |walker, _, item| {
                    walker.literal(item, None)
                }),
            },
            "$lookup" => {
                self.arguments(
                    argument,
                    name,
                    LOOKUP_ARGUMENTS,
                    |walker, key, item| match key {
                        "from" => walker.collection_reference(item, key),
                        "let" => walker.fields(item, key, Self::expression),
                        "pipeline" => walker.sub_pipeline(item),
                        _ => walker.literal(item, None),
                    },
                )
            }
            "$graphLookup" => self.arguments(
                argument,
                name,
                GRAPH_LOOKUP_ARGUMENTS,
                |walker, key, item| match key {
                    "from" => walker.collection_reference(item, key),
                    "startWith" => walker.expression(item),
                    "restrictSearchWithMatch" => walker.filter(item),
                    _ => walker.literal(item, None),
                },
            ),
            "$unionWith" => self.union_with(argument),
            "$facet" => self.fields(argument, name, Self::sub_pipeline),
            "$bucket" => self.arguments(argument, name, BUCKET_ARGUMENTS, |walker, key, item| {
                walker.bucket_argument(key, item)
            }),
            "$bucketAuto" => self.arguments(
                argument,
                name,
                BUCKET_AUTO_ARGUMENTS,
                |walker, key, item| walker.bucket_argument(key, item),
            ),
            "$replaceRoot" => {
                self.arguments(argument, name, REPLACE_ROOT_ARGUMENTS, |walker, _, item| {
                    walker.expression(item)
                })
            }
            "$sample" => self.arguments(argument, name, SAMPLE_ARGUMENTS, |walker, _, item| {
                walker.integer(item, "size", 1)
            }),
            "$densify" => self.arguments(argument, name, DENSIFY_ARGUMENTS, |walker, _, item| {
                walker.literal(item, None)
            }),
            "$fill" => self.arguments(
                argument,
                name,
                FILL_ARGUMENTS,
                |walker, key, item| match key {
                    "partitionBy" => walker.expression(item),
                    "sortBy" => walker.sort(item),
                    "output" => walker.fields(item, key, Self::fill_output),
                    _ => walker.literal(item, None),
                },
            ),
            "$setWindowFields" => self.arguments(
                argument,
                name,
                SET_WINDOW_FIELDS_ARGUMENTS,
                |walker, key, item| match key {
                    "partitionBy" => walker.expression(item),
                    "sortBy" => walker.sort(item),
                    _ => walker.fields(item, key, Self::window_output),
                },
            ),
            "$geoNear" => self.arguments(
                argument,
                name,
                GEO_NEAR_ARGUMENTS,
                |walker, key, item| match key {
                    "query" => walker.filter(item),
                    _ => walker.literal(item, None),
                },
            ),
            _ => self.expression(argument),
        }
    }

    fn arguments(
        &mut self,
        argument: &Value,
        owner: &str,
        allowed: &[&str],
        mut visit: impl FnMut(&mut Self, &str, &Value) -> Result<(), Refusal>,
    ) -> Result<(), Refusal> {
        let map = self.document(argument, owner)?;
        for (key, item) in map {
            self.enter_key(key, |walker| {
                if !allowed.contains(&key.as_str()) {
                    return Err(walker.invalid(&format!(
                        "{} is not an argument of {owner}",
                        super::echoable(key)
                    )));
                }
                visit(walker, key, item)
            })?;
        }
        Ok(())
    }

    fn fields(
        &mut self,
        argument: &Value,
        owner: &str,
        visit: fn(&mut Self, &Value) -> Result<(), Refusal>,
    ) -> Result<(), Refusal> {
        let map = self.document(argument, owner)?;
        for (key, item) in map {
            self.enter_key(key, |walker| {
                walker.output_name(key)?;
                visit(walker, item)
            })?;
        }
        Ok(())
    }

    fn output_name(&self, key: &str) -> Result<(), Refusal> {
        if key.starts_with('$') {
            return Err(self.not_allowed("mongo_operator_not_allowed", key, "field name"));
        }
        Ok(())
    }

    fn projection(&mut self, argument: &Value) -> Result<(), Refusal> {
        let map = self.document(argument, "a projection")?;
        for (key, item) in map {
            self.enter_key(key, |walker| {
                walker.output_name(key)?;
                match item {
                    Value::Number(_) | Value::Bool(_) => Ok(()),
                    Value::Object(inner)
                        if !inner.is_empty()
                            && !inner.keys().any(|inner_key| inner_key.starts_with('$')) =>
                    {
                        walker.projection(item)
                    }
                    _ => walker.expression(item),
                }
            })?;
        }
        Ok(())
    }

    fn sort(&mut self, argument: &Value) -> Result<(), Refusal> {
        self.fields(argument, "a sort", |walker, item| match item {
            Value::Number(_) => Ok(()),
            Value::Object(_) => walker.expression(item),
            _ => Err(walker.invalid("a sort direction must be 1, -1 or an expression")),
        })
    }

    fn hint(&mut self, argument: &Value) -> Result<(), Refusal> {
        match argument {
            Value::String(_) => Ok(()),
            _ => self.fields(argument, "a hint", |walker, item| {
                walker.literal(item, None)
            }),
        }
    }

    fn unset(&mut self, argument: &Value) -> Result<(), Refusal> {
        let valid = match argument {
            Value::String(_) => true,
            Value::Array(items) => items.iter().all(Value::is_string),
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(self.invalid("$unset takes a field name or an array of field names"))
        }
    }

    fn integer(&self, argument: &Value, name: &str, minimum: u64) -> Result<(), Refusal> {
        match argument.as_u64() {
            Some(value) if value >= minimum => Ok(()),
            _ => Err(self.invalid(&format!("{name} takes an integer of at least {minimum}"))),
        }
    }

    fn bucket_argument(&mut self, key: &str, item: &Value) -> Result<(), Refusal> {
        match key {
            "groupBy" => self.expression(item),
            "output" => self.fields(item, key, Self::expression),
            "boundaries" | "default" => self.literal(item, Some(super::LiteralRole::Other)),
            _ => self.literal(item, None),
        }
    }

    fn fill_output(&mut self, item: &Value) -> Result<(), Refusal> {
        self.arguments(
            item,
            "$fill output",
            FILL_OUTPUT_ARGUMENTS,
            |walker, key, value| match key {
                "value" => walker.expression(value),
                _ => walker.literal(value, None),
            },
        )
    }

    fn window_output(&mut self, item: &Value) -> Result<(), Refusal> {
        let map = self.document(item, "a window output")?;
        let mut operators = 0;
        for (key, value) in map {
            self.enter_key(key, |walker| {
                if key == WINDOW_ARGUMENT {
                    return walker.literal(value, None);
                }
                if !key.starts_with('$') {
                    return Err(walker.invalid("a window output holds one operator and its window"));
                }
                if !EXPRESSION_OPERATORS.contains(&key.as_str()) {
                    return Err(walker.not_allowed(
                        "mongo_operator_not_allowed",
                        key,
                        "window operator",
                    ));
                }
                walker.expression_operator(key, value)
            })?;
            operators += usize::from(key.starts_with('$'));
        }
        if operators != 1 {
            return Err(self.invalid("a window output holds exactly one operator"));
        }
        Ok(())
    }

    fn union_with(&mut self, argument: &Value) -> Result<(), Refusal> {
        match argument {
            Value::String(_) => self.collection_reference(argument, "$unionWith"),
            _ => {
                let map = self.document(argument, "$unionWith")?;
                for (key, item) in map {
                    self.enter_key(key, |walker| match key.as_str() {
                        "coll" => walker.collection_reference(item, key),
                        "pipeline" => walker.sub_pipeline(item),
                        "db" => Err(walker.cross_database("db")),
                        _ if !UNION_WITH_ARGUMENTS.contains(&key.as_str()) => Err(walker.invalid(
                            &format!("{} is not an argument of $unionWith", super::echoable(key)),
                        )),
                        _ => Ok(()),
                    })?;
                }
                Ok(())
            }
        }
    }

    fn collection_reference(&mut self, value: &Value, key: &str) -> Result<(), Refusal> {
        let Value::String(name) = value else {
            return Err(self.cross_database(key));
        };
        match collection_problem(name) {
            Some(NameProblem::System) => Err(system_refusal(&format!(
                "{key} names a system collection ({})",
                self.position()
            ))),
            Some(NameProblem::Invalid) => Err(self.refuse(
                "mongo_invalid_name",
                &format!("{key} is not a valid collection name"),
            )),
            None => Ok(()),
        }
    }

    fn cross_database(&self, key: &str) -> Refusal {
        self.refuse(
            "mongo_cross_database",
            &format!("{key} must name a collection of the same database, as a string"),
        )
        .with_hint(CROSS_DATABASE_HINT)
    }
}
