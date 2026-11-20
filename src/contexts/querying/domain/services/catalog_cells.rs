use serde::Deserialize;

use crate::contexts::querying::domain::types::Cell;

pub fn text(cell: &Cell) -> Option<String> {
    match cell {
        Cell::Text(value) => Some(value.clone()),
        _ => None,
    }
}

pub fn int(cell: &Cell) -> Option<i64> {
    match cell {
        Cell::Int(value) => Some(*value),
        _ => None,
    }
}

pub fn float(cell: &Cell) -> Option<f64> {
    match cell {
        Cell::Float(value) => Some(*value),
        Cell::Int(value) => Some(*value as f64),
        _ => None,
    }
}

pub fn json_cell<T: for<'de> Deserialize<'de>>(cell: &Cell) -> Vec<T> {
    match cell {
        Cell::Json(value) => serde_json::from_value(value.clone()).unwrap_or_default(),
        _ => Vec::new(),
    }
}
