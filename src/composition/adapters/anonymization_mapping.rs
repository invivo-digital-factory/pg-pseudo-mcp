use crate::contexts::anonymization::domain::types as anonymization;
use crate::contexts::querying::domain::types as querying;

pub fn cell(input: querying::Cell) -> anonymization::Cell {
    match input {
        querying::Cell::Null => anonymization::Cell::Null,
        querying::Cell::Bool(value) => anonymization::Cell::Bool(value),
        querying::Cell::Int(value) => anonymization::Cell::Int(value),
        querying::Cell::Float(value) => anonymization::Cell::Float(value),
        querying::Cell::Numeric(value) => anonymization::Cell::Numeric(value),
        querying::Cell::Text(value) => anonymization::Cell::Text(value),
        querying::Cell::Json(value) => anonymization::Cell::Json(value),
        querying::Cell::Uuid(value) => anonymization::Cell::Uuid(value),
        querying::Cell::Date(value) => anonymization::Cell::Date(value),
        querying::Cell::Time(value) => anonymization::Cell::Time(value),
        querying::Cell::Timestamp(value) => anonymization::Cell::Timestamp(value),
        querying::Cell::TimestampTz(value) => anonymization::Cell::TimestampTz(value),
        querying::Cell::Interval(value) => anonymization::Cell::Interval(value),
        querying::Cell::Inet(value) => anonymization::Cell::Inet(value),
        querying::Cell::Array(items) => {
            anonymization::Cell::Array(items.into_iter().map(cell).collect())
        }
        querying::Cell::Composite(fields) => anonymization::Cell::Composite(
            fields
                .into_iter()
                .map(|(name, field)| (name, cell(field)))
                .collect(),
        ),
        querying::Cell::Opaque { type_name, len } => anonymization::Cell::Opaque { type_name, len },
    }
}

pub fn column(column: &querying::ColumnMeta) -> anonymization::ColumnMeta {
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

pub fn lineage(lineage: querying::Lineage) -> anonymization::Lineage {
    match lineage {
        querying::Lineage::Clean => anonymization::Lineage::Clean,
        querying::Lineage::Personal => anonymization::Lineage::Personal,
        querying::Lineage::Unknown => anonymization::Lineage::Unknown,
    }
}

pub fn message_in(message: &querying::DbMessage) -> anonymization::DatabaseMessage {
    anonymization::DatabaseMessage {
        severity: message.severity.clone(),
        sqlstate: message.sqlstate.clone(),
        message: message.message.clone(),
        detail: message.detail.clone(),
        hint: message.hint.clone(),
        where_: message.where_.clone(),
    }
}

pub fn message_out(message: anonymization::DatabaseMessage) -> querying::DbMessage {
    querying::DbMessage {
        severity: message.severity,
        sqlstate: message.sqlstate,
        message: message.message,
        detail: message.detail,
        hint: message.hint,
        where_: message.where_,
    }
}

fn reason(reason: anonymization::Reason) -> querying::Reason {
    match reason {
        anonymization::Reason::Detected => querying::Reason::Detected,
        anonymization::Reason::Dictionary => querying::Reason::Dictionary,
        anonymization::Reason::Default => querying::Reason::Default,
        anonymization::Reason::Derived => querying::Reason::Derived,
    }
}

fn flag(flag: anonymization::ColumnFlag) -> querying::ColumnFlag {
    querying::ColumnFlag {
        kind: flag.kind,
        why: reason(flag.why),
    }
}

pub fn rows_out(rows: anonymization::AnonymizedRows) -> querying::AnonymizedRows {
    querying::AnonymizedRows {
        rows: rows.rows,
        pseudonymized: rows
            .pseudonymized
            .into_iter()
            .map(|(name, column_flag)| (name, flag(column_flag)))
            .collect(),
        per_column: rows
            .per_column
            .into_iter()
            .map(|column_flag| column_flag.map(flag))
            .collect(),
    }
}
