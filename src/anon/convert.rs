use crate::contexts::anonymization::domain::types as anonymization;
use crate::guard::Lineage;
use crate::pg::{Cell, ColumnMeta, DbMessage};

pub fn cell(input: Cell) -> anonymization::Cell {
    match input {
        Cell::Null => anonymization::Cell::Null,
        Cell::Bool(value) => anonymization::Cell::Bool(value),
        Cell::Int(value) => anonymization::Cell::Int(value),
        Cell::Float(value) => anonymization::Cell::Float(value),
        Cell::Numeric(value) => anonymization::Cell::Numeric(value),
        Cell::Text(value) => anonymization::Cell::Text(value),
        Cell::Json(value) => anonymization::Cell::Json(value),
        Cell::Uuid(value) => anonymization::Cell::Uuid(value),
        Cell::Date(value) => anonymization::Cell::Date(value),
        Cell::Time(value) => anonymization::Cell::Time(value),
        Cell::Timestamp(value) => anonymization::Cell::Timestamp(value),
        Cell::TimestampTz(value) => anonymization::Cell::TimestampTz(value),
        Cell::Interval(value) => anonymization::Cell::Interval(value),
        Cell::Inet(value) => anonymization::Cell::Inet(value),
        Cell::Array(items) => anonymization::Cell::Array(items.into_iter().map(cell).collect()),
        Cell::Composite(fields) => anonymization::Cell::Composite(
            fields
                .into_iter()
                .map(|(name, field)| (name, cell(field)))
                .collect(),
        ),
        Cell::Opaque { type_name, len } => anonymization::Cell::Opaque { type_name, len },
    }
}

pub fn column(column: &ColumnMeta) -> anonymization::ColumnMeta {
    anonymization::ColumnMeta {
        name: column.name.clone(),
        type_name: column.type_name.clone(),
        source: column
            .source
            .as_ref()
            .map(|source| anonymization::ColumnSource {
                schema: source.schema.clone(),
                table: source.table.clone(),
                column: source.column.clone(),
            }),
    }
}

pub fn lineage(lineage: Lineage) -> anonymization::Lineage {
    match lineage {
        Lineage::Clean => anonymization::Lineage::Clean,
        Lineage::Personal => anonymization::Lineage::Personal,
        Lineage::Unknown => anonymization::Lineage::Unknown,
    }
}

pub fn message_in(message: &DbMessage) -> anonymization::DatabaseMessage {
    anonymization::DatabaseMessage {
        severity: message.severity.clone(),
        sqlstate: message.sqlstate.clone(),
        message: message.message.clone(),
        detail: message.detail.clone(),
        hint: message.hint.clone(),
        where_: message.where_.clone(),
    }
}

pub fn message_out(message: anonymization::DatabaseMessage) -> DbMessage {
    DbMessage {
        severity: message.severity,
        sqlstate: message.sqlstate,
        message: message.message,
        detail: message.detail,
        hint: message.hint,
        where_: message.where_,
    }
}
