use std::collections::BTreeMap;
use std::time::Instant;

use rmcp::model::CallToolResult;
use rmcp::schemars;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::anon::apply::{Anonymizer, ColumnFlag};
use crate::contexts::connections::domain::errors::ConnectionError;
use crate::contexts::connections::domain::types::ExecutionLimits;
use crate::guard;
use crate::limits::QueryLimits;
use crate::pg::{DbMessage, PgError};
use crate::state::AppState;
use crate::tools::connection_error::tool_error;
use crate::tools::errors::{ErrorCategory, ToolError};
use crate::tools::result::{TruncatedBy, Truncation, Warning, json_result};

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct QueryArgs {
    #[schemars(description = "Connection name as returned by list_connections")]
    pub connection: String,
    #[schemars(description = "One read-only SQL statement")]
    pub sql: String,
    #[schemars(description = "Maximum rows returned (default 50, maximum 500)")]
    pub max_rows: Option<u64>,
    #[schemars(
        description = "Maximum size of the returned rows in bytes (default 16384, maximum 65536)"
    )]
    pub max_bytes: Option<u64>,
    #[schemars(description = "Statement timeout in milliseconds (default 10000, maximum 60000)")]
    pub timeout_ms: Option<u64>,
    #[schemars(
        description = "Characters to skip at the start of every text cell before the 500-character cap; repeat a query with the offset given in a cell truncation to read the rest of a long value"
    )]
    pub cell_offset: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct QueryResponse {
    pub columns: Vec<ColumnOut>,
    pub rows: Vec<Vec<Value>>,
    pub row_count: usize,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub pseudonymized: BTreeMap<String, ColumnFlag>,
    pub translated_literals: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub truncated: Vec<Truncation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<Warning>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<NoticeOut>,
    pub elapsed_ms: u128,
}

#[derive(Debug, Serialize)]
pub struct ColumnOut {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pseudonymized: Option<ColumnFlag>,
}

#[derive(Debug, Serialize)]
pub struct NoticeOut {
    pub severity: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

pub async fn query(state: &AppState, args: QueryArgs) -> CallToolResult {
    match run(state, args).await {
        Ok(response) => json_result(&response),
        Err(error) => error.into_call_result(),
    }
}

async fn run(state: &AppState, args: QueryArgs) -> Result<QueryResponse, ToolError> {
    let started = Instant::now();
    let pseudonymizer = state.pseudonymizer()?;
    let guarded = guard::prepare(&args.sql, &pseudonymizer)?;
    let (limits, clamps) =
        QueryLimits::from_request(args.max_rows, args.max_bytes, args.timeout_ms);
    let cell_offset = usize::try_from(args.cell_offset.unwrap_or(0)).unwrap_or(usize::MAX);
    let anonymizer = Anonymizer::new(&pseudonymizer, &guarded.injected);
    let output = state
        .connections
        .run_read_only_statement
        .execute(
            &args.connection,
            &guarded.sql,
            ExecutionLimits {
                max_rows: limits.max_rows,
                timeout_ms: limits.timeout_ms,
            },
        )
        .await
        .map_err(|error| database_error(&anonymizer, error, &limits))?;
    let mut warnings = guarded.warnings.clone();
    warnings.extend(clamps.into_iter().map(|clamp| {
        Warning::new(
            clamp.code,
            format!(
                "requested {} was capped to {}",
                clamp.requested, clamp.applied
            ),
        )
    }));
    if cell_offset > 0 {
        warnings.push(Warning::new(
            "cell_offset_applied",
            format!("every text cell starts at character {cell_offset}; shorter values are empty"),
        ));
    }
    let anonymized = anonymizer.rows(
        &output.columns,
        output.rows,
        guarded.derived_lineage.as_deref(),
    );
    let columns: Vec<ColumnOut> = output
        .columns
        .iter()
        .zip(&anonymized.per_column)
        .map(|(column, flag)| ColumnOut {
            name: column.name.clone(),
            type_name: column.type_name.clone(),
            pseudonymized: flag.clone(),
        })
        .collect();
    let mut truncated = Vec::new();
    let rows = bound_rows(
        anonymized.rows,
        &output.columns,
        &limits,
        cell_offset,
        &mut truncated,
    );
    if output.more_rows && !truncated.iter().any(|item| item.by == TruncatedBy::Bytes) {
        truncated.push(Truncation {
            by: TruncatedBy::Rows,
            limit: limits.max_rows as u64,
            column: None,
            hint: "narrow the filter or page with a keyset condition (WHERE id > last_id ORDER BY id) instead of OFFSET".into(),
        });
    }
    let notices = output
        .notices
        .iter()
        .map(|notice| notice_out(&anonymizer.message(notice)))
        .collect();
    Ok(QueryResponse {
        columns,
        row_count: rows.len(),
        rows,
        pseudonymized: anonymized.pseudonymized,
        translated_literals: guarded.translated_literals,
        truncated,
        warnings,
        notices,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

pub(crate) fn database_error(
    anonymizer: &Anonymizer,
    error: ConnectionError,
    limits: &QueryLimits,
) -> ToolError {
    tool_error(error, |error| session_error(anonymizer, error, limits))
}

fn session_error(anonymizer: &Anonymizer, error: PgError, limits: &QueryLimits) -> ToolError {
    match error {
        PgError::Timeout => ToolError::new(
            ErrorCategory::Timeout,
            "statement_timeout",
            format!("the statement ran longer than {} ms", limits.timeout_ms),
        )
        .with_hint("narrow the filter, check indexes with describe_table, or inspect the plan with explain")
        .retryable(),
        PgError::Db(message) => {
            let scrubbed = anonymizer.message(&message);
            let mut error = ToolError::new(ErrorCategory::SqlError, "sql_error", scrubbed.message);
            if let Some(sqlstate) = scrubbed.sqlstate {
                error = error.with_sqlstate(sqlstate);
            }
            match (scrubbed.detail, scrubbed.hint) {
                (Some(detail), Some(hint)) => error.with_hint(format!("{detail} {hint}")),
                (Some(text), None) | (None, Some(text)) => error.with_hint(text),
                (None, None) => error,
            }
        }
        PgError::Connect(message) => ToolError::new(
            ErrorCategory::ConnectionUnavailable,
            "connection_lost",
            anonymizer.free_text(&message),
        )
        .with_hint("retry; the server reconnects on the next call")
        .retryable(),
        PgError::Decode(message) => ToolError::new(
            ErrorCategory::WithheldFailClosed,
            "withheld_fail_closed",
            format!("a value could not be decoded safely and the result was withheld: {}", anonymizer.free_text(&message)),
        )
        .with_hint("select fewer columns or cast the problematic column to text"),
    }
}

fn notice_out(message: &DbMessage) -> NoticeOut {
    NoticeOut {
        severity: message.severity.clone(),
        message: message.message.clone(),
        detail: message.detail.clone(),
    }
}

fn bound_rows(
    rows: Vec<Vec<Value>>,
    columns: &[crate::pg::ColumnMeta],
    limits: &QueryLimits,
    cell_offset: usize,
    truncated: &mut Vec<Truncation>,
) -> Vec<Vec<Value>> {
    let mut kept = Vec::with_capacity(rows.len());
    let mut bytes = 0usize;
    let mut cut_column: Option<String> = None;
    for mut row in rows {
        for (index, value) in row.iter_mut().enumerate() {
            if cut_cell(value, cell_offset, limits.max_cell_chars) && cut_column.is_none() {
                cut_column = columns.get(index).map(|column| column.name.clone());
            }
        }
        let size = serde_json::to_string(&row).map_or(0, |text| text.len());
        if bytes + size > limits.max_bytes && !kept.is_empty() {
            truncated.push(Truncation {
                by: TruncatedBy::Bytes,
                limit: limits.max_bytes as u64,
                column: None,
                hint: "select fewer or narrower columns, or fetch fewer rows".into(),
            });
            break;
        }
        bytes += size;
        kept.push(row);
    }
    if let Some(column) = cut_column {
        truncated.push(Truncation {
            by: TruncatedBy::Cell,
            limit: limits.max_cell_chars as u64,
            column: Some(column),
            hint: format!(
                "long values are cut; repeat the same query with cell_offset={} to read the rest",
                cell_offset.saturating_add(limits.max_cell_chars)
            ),
        });
    }
    kept
}

fn cut_cell(value: &mut Value, offset: usize, max_chars: usize) -> bool {
    match value {
        Value::String(text) => {
            let remaining = text.chars().count().saturating_sub(offset);
            let kept: String = text.chars().skip(offset).take(max_chars).collect();
            *text = if remaining > max_chars {
                format!("{kept}…[+{} chars]", remaining - max_chars)
            } else {
                kept
            };
            remaining > max_chars
        }
        Value::Array(items) => {
            let mut cut = false;
            for item in items.iter_mut() {
                cut |= cut_cell(item, offset, max_chars);
            }
            cut
        }
        Value::Object(entries) => {
            let mut cut = false;
            for item in entries.values_mut() {
                cut |= cut_cell(item, offset, max_chars);
            }
            cut
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn column(name: &str) -> crate::pg::ColumnMeta {
        crate::pg::ColumnMeta {
            name: name.into(),
            type_name: "text".into(),
            type_oid: 25,
            source: None,
        }
    }

    #[test]
    fn an_offset_reads_the_next_slice_of_every_text_cell() {
        let long: String = ('a'..='z').cycle().take(1200).collect();
        let rows = vec![vec![json!(long), json!("short"), json!(7)]];
        let columns = [column("body"), column("label"), column("id")];
        let mut truncated = Vec::new();
        let kept = bound_rows(rows, &columns, &QueryLimits::default(), 500, &mut truncated);
        let expected: String = long.chars().skip(500).take(500).collect();
        assert_eq!(kept[0][0], json!(format!("{expected}…[+200 chars]")));
        assert_eq!(kept[0][1], json!(""));
        assert_eq!(kept[0][2], json!(7));
        assert_eq!(truncated.len(), 1);
        assert_eq!(truncated[0].column.as_deref(), Some("body"));
        assert!(truncated[0].hint.contains("cell_offset=1000"));
        assert!(!truncated[0].hint.contains("substr"));
    }

    #[test]
    fn the_last_slice_is_not_reported_as_cut() {
        let long: String = "x".repeat(1200);
        let mut truncated = Vec::new();
        let kept = bound_rows(
            vec![vec![json!(long)]],
            &[column("body")],
            &QueryLimits::default(),
            1000,
            &mut truncated,
        );
        assert_eq!(kept[0][0], json!("x".repeat(200)));
        assert!(truncated.is_empty());
    }
}
