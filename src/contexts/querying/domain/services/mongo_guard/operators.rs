use serde_json::{Map, Value};

use super::super::mongo_filter_walker::LiteralRole;
use super::Walker;
use super::extended_json::is_extended_json;
use crate::contexts::querying::domain::constants::mongo_policy::{
    EXPRESSION_OPERATORS, EXTENDED_JSON_REFUSED, FIELD_QUERY_OPERATORS, GEOMETRY_OPERATORS,
    META_KEYWORDS, TEXT_OPERATORS, TOP_LEVEL_QUERY_OPERATORS,
};
use crate::contexts::querying::domain::errors::Refusal;

impl Walker {
    pub(super) fn document<'a>(
        &mut self,
        value: &'a Value,
        what: &str,
    ) -> Result<&'a Map<String, Value>, Refusal> {
        let Value::Object(map) = value else {
            return Err(self.invalid(&format!("{what} must be an object")));
        };
        if map
            .keys()
            .any(|key| EXTENDED_JSON_REFUSED.contains(&key.as_str()))
        {
            self.extended_json(map)?;
        }
        if is_extended_json(map) {
            return Err(self.invalid(&format!(
                "{what} must be a document, not an extended JSON value"
            )));
        }
        Ok(map)
    }

    pub(super) fn filter(&mut self, value: &Value) -> Result<(), Refusal> {
        let map = self.document(value, "a filter")?;
        for (key, item) in map {
            self.enter_key(key, |walker| {
                if key.starts_with('$') {
                    walker.top_level_operator(key, item)
                } else {
                    walker.field_condition(item)
                }
            })?;
        }
        Ok(())
    }

    fn top_level_operator(&mut self, key: &str, item: &Value) -> Result<(), Refusal> {
        if !TOP_LEVEL_QUERY_OPERATORS.contains(&key) {
            return Err(self.not_allowed("mongo_operator_not_allowed", key, "query operator"));
        }
        match key {
            "$and" | "$or" | "$nor" => {
                let Some(branches) = item.as_array().filter(|items| !items.is_empty()) else {
                    return Err(self.invalid(&format!("{key} takes a non-empty array of filters")));
                };
                for (index, branch) in branches.iter().enumerate() {
                    self.enter_index(index, |walker| walker.filter(branch))?;
                }
                Ok(())
            }
            "$expr" => {
                let outer = self.in_expr;
                self.in_expr = true;
                let result = self.expression(item);
                self.in_expr = outer;
                result
            }
            "$text" => self.text(item),
            _ => {
                if !item.is_string() {
                    return Err(self.invalid("$comment takes a string"));
                }
                self.record(item, LiteralRole::Other);
                Ok(())
            }
        }
    }

    fn text(&mut self, item: &Value) -> Result<(), Refusal> {
        let map = self.document(item, "$text")?;
        for (key, value) in map {
            self.enter_key(key, |walker| {
                if !TEXT_OPERATORS.contains(&key.as_str()) {
                    return Err(walker.not_allowed(
                        "mongo_operator_not_allowed",
                        key,
                        "$text option",
                    ));
                }
                if key == "$search" {
                    if !value.is_string() {
                        return Err(walker.invalid("$search takes a string"));
                    }
                    walker.record(value, LiteralRole::Other);
                    return Ok(());
                }
                walker.literal(value, None)
            })?;
        }
        Ok(())
    }

    fn field_condition(&mut self, item: &Value) -> Result<(), Refusal> {
        match item {
            Value::Object(map) if is_extended_json(map) => self.extended_json(map),
            Value::Object(map) if map.keys().any(|key| key.starts_with('$')) => {
                self.field_operators(map)
            }
            Value::Object(_) | Value::Array(_) => self.literal(item, Some(LiteralRole::Other)),
            _ => {
                self.record(item, self.compared_role());
                Ok(())
            }
        }
    }

    fn compared(&mut self, item: &Value) -> Result<(), Refusal> {
        match item {
            Value::String(_) | Value::Number(_) => {
                self.record(item, self.compared_role());
                Ok(())
            }
            _ => self.literal(item, Some(LiteralRole::Other)),
        }
    }

    fn field_operators(&mut self, map: &Map<String, Value>) -> Result<(), Refusal> {
        for (key, argument) in map {
            self.enter_key(key, |walker| walker.field_operator(key, argument))?;
        }
        Ok(())
    }

    fn field_operator(&mut self, key: &str, argument: &Value) -> Result<(), Refusal> {
        if !key.starts_with('$') {
            return Err(self.invalid("an operator object cannot mix operators and field names"));
        }
        if !FIELD_QUERY_OPERATORS.contains(&key) {
            return Err(self.not_allowed("mongo_operator_not_allowed", key, "field operator"));
        }
        match key {
            "$eq" | "$ne" => self.compared(argument),
            "$gt" | "$gte" | "$lt" | "$lte" => self.literal(argument, Some(LiteralRole::Other)),
            "$in" | "$nin" => {
                let items = self.array(argument, key)?;
                for (index, item) in items.iter().enumerate() {
                    self.enter_index(index, |walker| walker.compared(item))?;
                }
                Ok(())
            }
            "$all" => {
                let items = self.array(argument, key)?;
                for (index, item) in items.iter().enumerate() {
                    self.enter_index(index, |walker| match item {
                        Value::Object(map) if map.len() == 1 && map.contains_key("$elemMatch") => {
                            walker.enter_key("$elemMatch", |walker| {
                                walker.elem_match(&map["$elemMatch"])
                            })
                        }
                        _ => walker.compared(item),
                    })?;
                }
                Ok(())
            }
            "$not" => match argument {
                Value::Object(map) if is_extended_json(map) => self.extended_json(map),
                Value::Object(map) if !map.is_empty() => self.field_operators(map),
                _ => Err(self.invalid("$not takes an operator object or a regular expression")),
            },
            "$regex" => match argument {
                Value::String(_) => {
                    self.record(argument, LiteralRole::Pattern);
                    Ok(())
                }
                Value::Object(map) if is_extended_json(map) => self.extended_json(map),
                _ => Err(self.invalid("$regex takes a string or a regular expression")),
            },
            "$elemMatch" => self.elem_match(argument),
            "$geoWithin" | "$geoIntersects" | "$near" | "$nearSphere" => self.geometry(argument),
            _ => self.literal(argument, None),
        }
    }

    fn elem_match(&mut self, argument: &Value) -> Result<(), Refusal> {
        let map = self.document(argument, "$elemMatch")?;
        let operator_form = map.keys().next().is_some_and(|first| {
            first.starts_with('$') && !TOP_LEVEL_QUERY_OPERATORS.contains(&first.as_str())
        });
        if operator_form {
            self.field_operators(map)
        } else {
            self.filter(argument)
        }
    }

    fn geometry(&mut self, argument: &Value) -> Result<(), Refusal> {
        match argument {
            Value::Object(map) if !is_extended_json(map) => {
                for (key, value) in map {
                    self.enter_key(key, |walker| {
                        if !GEOMETRY_OPERATORS.contains(&key.as_str()) {
                            return Err(walker.not_allowed(
                                "mongo_operator_not_allowed",
                                key,
                                "geometry operator",
                            ));
                        }
                        walker.literal(value, None)
                    })?;
                }
                Ok(())
            }
            _ => self.literal(argument, None),
        }
    }

    pub(super) fn array<'a>(
        &self,
        argument: &'a Value,
        operator: &str,
    ) -> Result<&'a Vec<Value>, Refusal> {
        argument
            .as_array()
            .ok_or_else(|| self.invalid(&format!("{} takes an array", super::echoable(operator))))
    }

    pub(super) fn literal(
        &mut self,
        value: &Value,
        role: Option<LiteralRole>,
    ) -> Result<(), Refusal> {
        match value {
            Value::Object(map) if is_extended_json(map) => self.extended_json(map),
            Value::Object(map) => {
                for (key, item) in map {
                    self.enter_key(key, |walker| {
                        if key.starts_with('$') {
                            return Err(walker.not_allowed(
                                "mongo_operator_not_allowed",
                                key,
                                "key in a literal value",
                            ));
                        }
                        walker.literal(item, role)
                    })?;
                }
                Ok(())
            }
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    self.enter_index(index, |walker| walker.literal(item, role))?;
                }
                Ok(())
            }
            _ => {
                if let Some(role) = role {
                    self.record(value, role);
                }
                Ok(())
            }
        }
    }

    pub(super) fn expression(&mut self, value: &Value) -> Result<(), Refusal> {
        match value {
            Value::String(text) if text.starts_with('$') => Ok(()),
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    self.enter_index(index, |walker| walker.expression(item))?;
                }
                Ok(())
            }
            Value::Object(map) if is_extended_json(map) => self.extended_json(map),
            Value::Object(map) => {
                let operators: Vec<&String> =
                    map.keys().filter(|key| key.starts_with('$')).collect();
                if operators.is_empty() {
                    for (key, item) in map {
                        self.enter_key(key, |walker| walker.expression(item))?;
                    }
                    return Ok(());
                }
                for operator in &operators {
                    if !EXPRESSION_OPERATORS.contains(&operator.as_str()) {
                        return Err(self.enter_key(operator, |walker| {
                            Ok(walker.not_allowed(
                                "mongo_operator_not_allowed",
                                operator,
                                "expression operator",
                            ))
                        })?);
                    }
                }
                let [operator] = operators.as_slice() else {
                    return Err(self.invalid("an expression object must hold exactly one operator"));
                };
                if map.len() != 1 {
                    return Err(self.invalid(
                        "an expression operator cannot share its object with field names",
                    ));
                }
                self.enter_key(operator, |walker| {
                    walker.expression_operator(operator, &map[operator.as_str()])
                })
            }
            _ => {
                self.record(value, LiteralRole::Other);
                Ok(())
            }
        }
    }

    pub(super) fn expression_operator(
        &mut self,
        operator: &str,
        argument: &Value,
    ) -> Result<(), Refusal> {
        match operator {
            "$literal" => self.literal(argument, Some(LiteralRole::Other)),
            "$meta" => match argument.as_str() {
                Some(keyword) if META_KEYWORDS.contains(&keyword) => Ok(()),
                _ => Err(self.invalid(&format!("$meta takes one of {}", META_KEYWORDS.join(", ")))),
            },
            "$eq" | "$ne" if self.in_expr => self.expression_equality(argument),
            "$in" if self.in_expr => self.expression_membership(argument),
            "$regexMatch" | "$regexFind" | "$regexFindAll" => match argument {
                Value::Object(map) if !map.keys().any(|key| key.starts_with('$')) => {
                    for (key, item) in map {
                        self.enter_key(key, |walker| {
                            if key == "regex" && item.is_string() {
                                walker.record(item, LiteralRole::Pattern);
                                Ok(())
                            } else {
                                walker.expression(item)
                            }
                        })?;
                    }
                    Ok(())
                }
                _ => self.expression(argument),
            },
            _ => self.expression(argument),
        }
    }

    fn expression_equality(&mut self, argument: &Value) -> Result<(), Refusal> {
        match argument.as_array().map(Vec::as_slice) {
            Some([left, right]) if is_field_path(left) && is_compared_literal(right) => self
                .enter_index(1, |walker| {
                    walker.record(right, LiteralRole::ComparedInExpression);
                    Ok(())
                }),
            Some([left, right]) if is_compared_literal(left) && is_field_path(right) => self
                .enter_index(0, |walker| {
                    walker.record(left, LiteralRole::ComparedInExpression);
                    Ok(())
                }),
            _ => self.expression(argument),
        }
    }

    fn expression_membership(&mut self, argument: &Value) -> Result<(), Refusal> {
        match argument.as_array().map(Vec::as_slice) {
            Some([left, Value::Array(items)]) if is_field_path(left) => {
                self.enter_index(1, |walker| {
                    for (index, item) in items.iter().enumerate() {
                        walker.enter_index(index, |walker| {
                            if is_compared_literal(item) {
                                walker.record(item, LiteralRole::ComparedInExpression);
                                Ok(())
                            } else {
                                walker.expression(item)
                            }
                        })?;
                    }
                    Ok(())
                })
            }
            Some([left, right]) if is_compared_literal(left) && is_field_path(right) => self
                .enter_index(0, |walker| {
                    walker.record(left, LiteralRole::ComparedInExpression);
                    Ok(())
                }),
            _ => self.expression(argument),
        }
    }
}

fn is_field_path(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|text| text.starts_with('$') && !text.starts_with("$$") && text.len() > 1)
}

fn is_compared_literal(value: &Value) -> bool {
    match value {
        Value::String(text) => !text.starts_with('$'),
        Value::Number(number) => number.is_i64() || number.is_u64(),
        _ => false,
    }
}
