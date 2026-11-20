use crate::contexts::connections::domain::types as connections;
use crate::contexts::querying::domain::types as querying;

pub fn cell(input: connections::Cell) -> querying::Cell {
    match input {
        connections::Cell::Null => querying::Cell::Null,
        connections::Cell::Bool(value) => querying::Cell::Bool(value),
        connections::Cell::Int(value) => querying::Cell::Int(value),
        connections::Cell::Float(value) => querying::Cell::Float(value),
        connections::Cell::Numeric(value) => querying::Cell::Numeric(value),
        connections::Cell::Text(value) => querying::Cell::Text(value),
        connections::Cell::Json(value) => querying::Cell::Json(value),
        connections::Cell::Uuid(value) => querying::Cell::Uuid(value),
        connections::Cell::Date(value) => querying::Cell::Date(value),
        connections::Cell::Time(value) => querying::Cell::Time(value),
        connections::Cell::Timestamp(value) => querying::Cell::Timestamp(value),
        connections::Cell::TimestampTz(value) => querying::Cell::TimestampTz(value),
        connections::Cell::Interval(value) => querying::Cell::Interval(value),
        connections::Cell::Inet(value) => querying::Cell::Inet(value),
        connections::Cell::Array(items) => {
            querying::Cell::Array(items.into_iter().map(cell).collect())
        }
        connections::Cell::Composite(fields) => querying::Cell::Composite(
            fields
                .into_iter()
                .map(|(name, field)| (name, cell(field)))
                .collect(),
        ),
        connections::Cell::Opaque { type_name, len } => querying::Cell::Opaque { type_name, len },
    }
}

fn column(column: connections::ColumnMeta) -> querying::ColumnMeta {
    querying::ColumnMeta {
        name: column.name,
        type_name: column.type_name,
        source: column.source.map(|source| querying::ColumnSource {
            schema: source.schema,
            table: source.table,
            column: source.column,
        }),
    }
}

pub fn db_message(message: connections::DbMessage) -> querying::DbMessage {
    querying::DbMessage {
        severity: message.severity,
        sqlstate: message.sqlstate,
        message: message.message,
        detail: message.detail,
        hint: message.hint,
        where_: message.where_,
    }
}

pub fn query_output(output: connections::QueryOutput) -> querying::QueryOutput {
    querying::QueryOutput {
        columns: output.columns.into_iter().map(column).collect(),
        rows: output
            .rows
            .into_iter()
            .map(|row| row.into_iter().map(cell).collect())
            .collect(),
        more_rows: output.more_rows,
        notices: output.notices.into_iter().map(db_message).collect(),
    }
}

pub fn database_listing(listing: connections::DatabaseListing) -> querying::DatabaseListing {
    querying::DatabaseListing {
        databases: listing.databases,
        default: listing.default,
        truncated: listing.truncated,
    }
}

pub fn execution_limits(limits: querying::ExecutionLimits) -> connections::ExecutionLimits {
    connections::ExecutionLimits {
        max_rows: limits.max_rows,
        timeout_ms: limits.timeout_ms,
    }
}
