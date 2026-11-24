use serde_json::{Map, Value};

use super::super::mongo_filter_walker::LiteralRole;
use super::Walker;
use crate::contexts::querying::domain::constants::mongo_policy::{
    EXTENDED_JSON_REFUSED, EXTENDED_JSON_TYPES,
};
use crate::contexts::querying::domain::errors::Refusal;

pub(super) fn is_extended_json(map: &Map<String, Value>) -> bool {
    map.keys().any(|key| is_extended_json_key(key))
}

fn is_extended_json_key(key: &str) -> bool {
    EXTENDED_JSON_TYPES.contains(&key) || EXTENDED_JSON_REFUSED.contains(&key)
}

impl Walker {
    pub(super) fn extended_json(&mut self, map: &Map<String, Value>) -> Result<(), Refusal> {
        if let Some(refused) = map
            .keys()
            .find(|key| EXTENDED_JSON_REFUSED.contains(&key.as_str()))
        {
            return Err(self.enter_key(refused, |walker| {
                Ok(walker.not_allowed(
                    "mongo_extended_json_not_allowed",
                    refused,
                    "extended JSON type",
                ))
            })?);
        }
        let Some(name) = map.keys().find(|key| is_extended_json_key(key)) else {
            return Ok(());
        };
        let body = &map[name];
        let valid = map.len() == 1
            && match name.as_str() {
                "$oid" => text(body).is_some_and(is_object_id),
                "$numberInt" => text(body).is_some_and(|text| text.parse::<i32>().is_ok()),
                "$numberLong" => text(body).is_some_and(|text| text.parse::<i64>().is_ok()),
                "$numberDouble" => text(body).is_some_and(is_double),
                "$numberDecimal" => text(body).is_some_and(is_decimal),
                "$binary" => is_binary(body),
                "$uuid" => text(body).is_some_and(is_uuid),
                "$date" => is_date(body),
                "$timestamp" => is_timestamp(body),
                "$regularExpression" => is_regular_expression(body),
                "$minKey" | "$maxKey" => body.as_u64() == Some(1),
                _ => false,
            };
        self.enter_key(name, |walker| {
            if !valid {
                return Err(walker.refuse(
                    "mongo_extended_json_invalid",
                    &format!("{name} is not a well-formed extended JSON value"),
                ));
            }
            if let Some(pattern) = body.get("pattern").filter(|_| name == "$regularExpression") {
                walker.enter_key("pattern", |walker| {
                    walker.record(pattern, LiteralRole::Pattern);
                    Ok(())
                })?;
            }
            Ok(())
        })
    }
}

fn text(value: &Value) -> Option<&str> {
    value.as_str()
}

fn is_object_id(text: &str) -> bool {
    text.len() == 24 && text.chars().all(|character| character.is_ascii_hexdigit())
}

fn is_double(text: &str) -> bool {
    matches!(text, "Infinity" | "-Infinity" | "NaN")
        || (text.parse::<f64>().is_ok() && text.chars().any(|character| character.is_ascii_digit()))
}

fn is_decimal(text: &str) -> bool {
    matches!(text, "Infinity" | "-Infinity" | "NaN")
        || (!text.is_empty()
            && text.chars().any(|character| character.is_ascii_digit())
            && text.chars().all(|character| {
                character.is_ascii_digit() || matches!(character, '.' | '-' | '+' | 'e' | 'E')
            }))
}

fn is_uuid(text: &str) -> bool {
    text.len() == 36
        && text.char_indices().all(|(index, character)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                character == '-'
            } else {
                character.is_ascii_hexdigit()
            }
        })
}

fn exact<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Map<String, Value>> {
    let map = value.as_object()?;
    (map.len() == keys.len() && keys.iter().all(|key| map.contains_key(*key))).then_some(map)
}

fn is_binary(body: &Value) -> bool {
    exact(body, &["base64", "subType"]).is_some_and(|map| {
        map["base64"].as_str().is_some_and(is_base64)
            && map["subType"].as_str().is_some_and(|subtype| {
                (1..=2).contains(&subtype.len())
                    && subtype
                        .chars()
                        .all(|character| character.is_ascii_hexdigit())
            })
    })
}

fn is_base64(text: &str) -> bool {
    text.len().is_multiple_of(4)
        && text.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '/' | '=')
        })
}

fn is_date(body: &Value) -> bool {
    match body {
        Value::String(text) => !text.is_empty(),
        Value::Number(number) => number.is_i64(),
        Value::Object(_) => exact(body, &["$numberLong"])
            .and_then(|map| map["$numberLong"].as_str())
            .is_some_and(|text| text.parse::<i64>().is_ok()),
        _ => false,
    }
}

fn is_timestamp(body: &Value) -> bool {
    exact(body, &["t", "i"]).is_some_and(|map| {
        ["t", "i"].iter().all(|key| {
            map[*key]
                .as_u64()
                .is_some_and(|part| u32::try_from(part).is_ok())
        })
    })
}

fn is_regular_expression(body: &Value) -> bool {
    exact(body, &["pattern", "options"]).is_some_and(|map| {
        map["pattern"]
            .as_str()
            .is_some_and(|pattern| !pattern.contains('\0'))
            && map["options"]
                .as_str()
                .is_some_and(|options| options.chars().all(|option| "ilmsux".contains(option)))
    })
}
