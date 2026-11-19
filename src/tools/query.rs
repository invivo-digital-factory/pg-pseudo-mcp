use std::collections::BTreeMap;
use std::time::Instant;

use rmcp::model::CallToolResult;
use rmcp::schemars;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::anon::apply::{Anonymizer, ColumnFlag};
use crate::guard;
use crate::limits::QueryLimits;
use crate::pg::{DbMessage, PgError};
use crate::state::AppState;
use crate::tools::errors::{ErrorCategory, ToolError};
use crate::tools::result::{TruncatedBy, Truncation, Warning, json_result};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct QueryArgs {
    #[schemars(description = "Connection name as returned by list_connections")]
    pub connection: String,
    #[schemars(description = "One read-only SQL statement")]
    pub sql: String,
    #[schemars(description = "Maximum rows returned (default 50, maximum 500)")]
    pub max_rows: Option<u64>,
    #[schemars(description = "Statement timeout in milliseconds (default 10000, maximum 60000)")]
    pub timeout_ms: Option<u64>,
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
    let guarded = guard::prepare(&args.sql, pseudonymizer.as_ref())?;
    let (limits, clamps) = QueryLimits::from_request(args.max_rows, args.timeout_ms);
    let session = state.session(&args.connection).await?;
    let anonymizer = Anonymizer::new(&pseudonymizer, &guarded.injected);
    let output = {
        let mut session = session.lock().await;
        session
            .run_read_only(&guarded.sql, limits.max_rows, limits.timeout_ms)
            .await
            .map_err(|error| database_error(&anonymizer, error, &limits))?
    };
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
    let columns: Vec<ColumnOut> = output
        .columns
        .iter()
        .map(|column| ColumnOut {
            name: column.name.clone(),
            type_name: column.type_name.clone(),
        })
        .collect();
    let anonymized = anonymizer.rows(&output.columns, output.rows);
    let mut truncated = Vec::new();
    let rows = bound_rows(anonymized.rows, &output.columns, &limits, &mut truncated);
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
    error: PgError,
    limits: &QueryLimits,
) -> ToolError {
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
    truncated: &mut Vec<Truncation>,
) -> Vec<Vec<Value>> {
    let mut kept = Vec::with_capacity(rows.len());
    let mut bytes = 0usize;
    let mut cut_column: Option<String> = None;
    for mut row in rows {
        for (index, value) in row.iter_mut().enumerate() {
            if cut_cell(value, limits.max_cell_chars) && cut_column.is_none() {
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
            hint: "long values are cut; select substr(column, start, length) to read a slice"
                .into(),
        });
    }
    kept
}

fn cut_cell(value: &mut Value, max_chars: usize) -> bool {
    match value {
        Value::String(text) => {
            let length = text.chars().count();
            if length <= max_chars {
                return false;
            }
            let kept: String = text.chars().take(max_chars).collect();
            *text = format!("{kept}…[+{} chars]", length - max_chars);
            true
        }
        Value::Array(items) => items
            .iter_mut()
            .fold(false, |cut, item| cut_cell(item, max_chars) || cut),
        Value::Object(entries) => entries
            .values_mut()
            .fold(false, |cut, item| cut_cell(item, max_chars) || cut),
        _ => false,
    }
}
