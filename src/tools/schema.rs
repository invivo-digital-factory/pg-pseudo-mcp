use rmcp::model::CallToolResult;
use rmcp::schemars;
use serde::{Deserialize, Serialize};

use crate::anon::apply::Anonymizer;
use crate::anon::{ColumnInfo, classify_column};
use crate::limits::QueryLimits;
use crate::pg::{Cell, QueryOutput};
use crate::state::AppState;
use crate::tools::errors::{ErrorCategory, ToolError};
use crate::tools::query::database_error;
use crate::tools::result::json_result;

const SEARCH_LIMIT: usize = 100;
const DEFAULT_LIST_LIMIT: u32 = 100;
const MAX_LIST_LIMIT: u32 = 500;
const SUGGESTION_POOL: usize = 10_000;
const MAX_SUGGESTIONS: usize = 5;
const MAX_COMMENT_CHARS: usize = 200;
const RELATION_KINDS: &str = "c.relkind IN ('r', 'v', 'm', 'p', 'f')";
const USER_SCHEMAS: &str =
    "n.nspname NOT IN ('pg_catalog', 'information_schema') AND n.nspname !~ '^pg_(toast|temp_)'";
const DETECTED_PER_VALUE: &str = "detected_per_value";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchSchemaArgs {
    #[schemars(description = "Connection name as returned by list_connections")]
    pub connection: String,
    #[schemars(description = "Case-insensitive substring matched against table and column names")]
    pub pattern: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListTablesArgs {
    #[schemars(description = "Connection name as returned by list_connections")]
    pub connection: String,
    #[schemars(description = "Restrict to one schema")]
    pub schema: Option<String>,
    #[schemars(description = "Case-insensitive substring matched against table names")]
    pub name_like: Option<String>,
    #[schemars(description = "Maximum number of tables returned (default 100, maximum 500)")]
    pub limit: Option<u32>,
    #[schemars(description = "Number of tables to skip, for pagination")]
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DescribeTableArgs {
    #[schemars(description = "Connection name as returned by list_connections")]
    pub connection: String,
    #[schemars(description = "Table name, optionally schema-qualified (schema.table)")]
    pub table: String,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub matches: Vec<SchemaMatch>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
pub struct SchemaMatch {
    pub schema: String,
    pub table: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub tables: Vec<TableEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_offset: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct TableEntry {
    pub schema: String,
    pub name: String,
    pub kind: &'static str,
    pub estimated_rows: Option<i64>,
    pub columns: i64,
}

#[derive(Debug, Serialize)]
pub struct DescribeResponse {
    pub schema: String,
    pub name: String,
    pub kind: &'static str,
    pub estimated_rows: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    pub columns: Vec<ColumnEntry>,
    pub primary_key: Vec<String>,
    pub foreign_keys: Vec<ForeignKey>,
    pub referenced_by: Vec<Reference>,
    pub indexes: Vec<IndexEntry>,
}

#[derive(Debug, Serialize)]
pub struct ColumnEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub not_null: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pseudonymized: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ForeignKey {
    pub columns: Vec<String>,
    pub references: String,
}

#[derive(Debug, Serialize)]
pub struct Reference {
    pub table: String,
    pub columns: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct IndexEntry {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub partial: bool,
}

#[derive(Debug, Deserialize)]
struct RawColumn {
    name: String,
    #[serde(rename = "type")]
    type_name: String,
    not_null: bool,
    comment: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawForeignKey {
    columns: Vec<String>,
    ref_schema: String,
    ref_table: String,
    ref_columns: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawReference {
    schema: String,
    table: String,
    columns: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawIndex {
    name: String,
    unique: bool,
    partial: bool,
    columns: Vec<Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    oid: i64,
    schema: String,
    name: String,
    path_rank: Option<i64>,
}

pub async fn search_schema(state: &AppState, args: SearchSchemaArgs) -> CallToolResult {
    respond(run_search(state, args).await)
}

pub async fn list_tables(state: &AppState, args: ListTablesArgs) -> CallToolResult {
    respond(run_list(state, args).await)
}

pub async fn describe_table(state: &AppState, args: DescribeTableArgs) -> CallToolResult {
    respond(run_describe(state, args).await)
}

fn respond<T: Serialize>(result: Result<T, ToolError>) -> CallToolResult {
    match result {
        Ok(response) => json_result(&response),
        Err(error) => error.into_call_result(),
    }
}

struct Catalog<'a> {
    state: &'a AppState,
    connection: &'a str,
    anonymizer: Anonymizer<'a>,
}

impl Catalog<'_> {
    async fn fetch(&self, sql: &str, max_rows: usize) -> Result<QueryOutput, ToolError> {
        let limits = QueryLimits::default();
        let mut session = self.state.session(self.connection).await?;
        session
            .run_read_only(sql, max_rows, limits.timeout_ms)
            .await
            .map_err(|error| database_error(&self.anonymizer, error, &limits))
    }
}

async fn run_search(state: &AppState, args: SearchSchemaArgs) -> Result<SearchResponse, ToolError> {
    let pseudonymizer = state.pseudonymizer()?;
    let catalog = Catalog {
        state,
        connection: &args.connection,
        anonymizer: Anonymizer::new(&pseudonymizer, &[]),
    };
    let pattern = literal(&args.pattern);
    let sql = format!(
        "WITH rels AS (\
           SELECT c.oid, n.nspname::text AS schema_name, c.relname::text AS table_name \
           FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
           WHERE {RELATION_KINDS} AND {USER_SCHEMAS} AND NOT c.relispartition) \
         SELECT schema_name, table_name, NULL::text AS column_name, NULL::text AS type_name, 0::int8 AS ordinal \
         FROM rels WHERE strpos(lower(table_name), lower({pattern})) > 0 \
         UNION ALL \
         SELECT r.schema_name, r.table_name, a.attname::text, format_type(a.atttypid, a.atttypmod), a.attnum::int8 \
         FROM rels r JOIN pg_attribute a ON a.attrelid = r.oid AND a.attnum > 0 AND NOT a.attisdropped \
         WHERE strpos(lower(a.attname::text), lower({pattern})) > 0 \
         ORDER BY schema_name, table_name, ordinal"
    );
    let output = catalog.fetch(&sql, SEARCH_LIMIT).await?;
    let matches = output
        .rows
        .iter()
        .map(|row| SchemaMatch {
            schema: text(&row[0]).unwrap_or_default(),
            table: text(&row[1]).unwrap_or_default(),
            column: text(&row[2]),
            type_name: text(&row[3]).map(|type_name| short_type(&type_name)),
        })
        .collect();
    Ok(SearchResponse {
        matches,
        truncated: output.more_rows,
    })
}

async fn run_list(state: &AppState, args: ListTablesArgs) -> Result<ListResponse, ToolError> {
    let pseudonymizer = state.pseudonymizer()?;
    let catalog = Catalog {
        state,
        connection: &args.connection,
        anonymizer: Anonymizer::new(&pseudonymizer, &[]),
    };
    let limit = args
        .limit
        .unwrap_or(DEFAULT_LIST_LIMIT)
        .clamp(1, MAX_LIST_LIMIT);
    let offset = args.offset.unwrap_or(0);
    let mut filters = String::new();
    if let Some(schema) = &args.schema {
        filters.push_str(&format!(" AND n.nspname::text = {}", literal(schema)));
    }
    if let Some(name) = &args.name_like {
        filters.push_str(&format!(
            " AND strpos(lower(c.relname::text), lower({})) > 0",
            literal(name)
        ));
    }
    let sql = format!(
        "SELECT n.nspname::text, c.relname::text, c.relkind::text, c.reltuples::float8, \
           (SELECT count(*) FROM pg_attribute a WHERE a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped)::int8 \
         FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE {RELATION_KINDS} AND {USER_SCHEMAS} AND NOT c.relispartition{filters} \
         ORDER BY 1, 2 OFFSET {offset}"
    );
    let output = catalog.fetch(&sql, limit as usize).await?;
    let tables = output
        .rows
        .iter()
        .map(|row| {
            let kind = relation_kind(&text(&row[2]).unwrap_or_default());
            TableEntry {
                schema: text(&row[0]).unwrap_or_default(),
                name: text(&row[1]).unwrap_or_default(),
                kind,
                estimated_rows: estimate(kind, float(&row[3])),
                columns: int(&row[4]).unwrap_or(0),
            }
        })
        .collect();
    Ok(ListResponse {
        tables,
        next_offset: output
            .more_rows
            .then(|| u64::from(offset) + u64::from(limit)),
    })
}

async fn run_describe(
    state: &AppState,
    args: DescribeTableArgs,
) -> Result<DescribeResponse, ToolError> {
    let pseudonymizer = state.pseudonymizer()?;
    let catalog = Catalog {
        state,
        connection: &args.connection,
        anonymizer: Anonymizer::new(&pseudonymizer, &[]),
    };
    let (schema, name) = split_qualified(&args.table);
    let target = resolve(&catalog, &args.table, schema.as_deref(), &name).await?;
    let output = catalog.fetch(&describe_sql(target.oid), 1).await?;
    let row = output.rows.first().ok_or_else(|| {
        ToolError::new(
            ErrorCategory::UnknownTable,
            "unknown_table",
            format!("table {} disappeared while it was described", args.table),
        )
        .retryable()
    })?;
    let anonymizer = &catalog.anonymizer;
    let kind = relation_kind(&text(&row[2]).unwrap_or_default());
    let columns: Vec<RawColumn> = json_cell(&row[5]);
    let foreign_keys: Vec<RawForeignKey> = json_cell(&row[7]);
    let referenced_by: Vec<RawReference> = json_cell(&row[8]);
    let indexes: Vec<RawIndex> = json_cell(&row[9]);
    Ok(DescribeResponse {
        schema: text(&row[0]).unwrap_or_default(),
        name: text(&row[1]).unwrap_or_default(),
        kind,
        estimated_rows: estimate(kind, float(&row[3])),
        comment: text(&row[4]).map(|comment| scrub_comment(anonymizer, &comment)),
        columns: columns
            .into_iter()
            .map(|column| column_entry(anonymizer, column))
            .collect(),
        primary_key: json_cell(&row[6]),
        foreign_keys: foreign_keys
            .into_iter()
            .map(|fk| ForeignKey {
                references: format!(
                    "{}.{}({})",
                    fk.ref_schema,
                    fk.ref_table,
                    fk.ref_columns.join(", ")
                ),
                columns: fk.columns,
            })
            .collect(),
        referenced_by: referenced_by
            .into_iter()
            .map(|reference| Reference {
                table: format!("{}.{}", reference.schema, reference.table),
                columns: reference.columns,
            })
            .collect(),
        indexes: indexes
            .into_iter()
            .map(|index| IndexEntry {
                name: index.name,
                unique: index.unique,
                partial: index.partial,
                columns: index
                    .columns
                    .into_iter()
                    .map(|column| column.unwrap_or_else(|| "(expression)".into()))
                    .collect(),
            })
            .collect(),
    })
}

async fn resolve(
    catalog: &Catalog<'_>,
    requested: &str,
    schema: Option<&str>,
    name: &str,
) -> Result<Candidate, ToolError> {
    let schema_filter = schema
        .map(|schema| format!(" AND lower(n.nspname::text) = lower({})", literal(schema)))
        .unwrap_or_default();
    let sql = format!(
        "SELECT c.oid::int8, n.nspname::text, c.relname::text, \
           array_position(current_schemas(false), n.nspname)::int8 \
         FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE {RELATION_KINDS} AND {USER_SCHEMAS} \
           AND lower(c.relname::text) = lower({}){schema_filter}",
        literal(name)
    );
    let output = catalog.fetch(&sql, SUGGESTION_POOL).await?;
    let candidates: Vec<Candidate> = output
        .rows
        .iter()
        .map(|row| Candidate {
            oid: int(&row[0]).unwrap_or_default(),
            schema: text(&row[1]).unwrap_or_default(),
            name: text(&row[2]).unwrap_or_default(),
            path_rank: int(&row[3]),
        })
        .collect();
    match pick(candidates, schema, name) {
        Pick::One(candidate) => Ok(candidate),
        Pick::Ambiguous(names) => Err(ToolError::new(
            ErrorCategory::UnknownTable,
            "ambiguous_table",
            format!(
                "{requested} exists in several schemas: {}",
                names.join(", ")
            ),
        )
        .with_hint("qualify the table with its schema (schema.table)")),
        Pick::None => Err(unknown_table(catalog, requested, name).await),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Pick {
    One(Candidate),
    Ambiguous(Vec<String>),
    None,
}

fn pick(candidates: Vec<Candidate>, schema: Option<&str>, name: &str) -> Pick {
    let exact: Vec<Candidate> = candidates
        .iter()
        .filter(|candidate| {
            candidate.name == name && schema.is_none_or(|schema| candidate.schema == schema)
        })
        .cloned()
        .collect();
    let mut pool = if exact.is_empty() { candidates } else { exact };
    pool.sort_by(|left, right| {
        (
            left.path_rank.is_none(),
            left.path_rank,
            &left.schema,
            &left.name,
        )
            .cmp(&(
                right.path_rank.is_none(),
                right.path_rank,
                &right.schema,
                &right.name,
            ))
    });
    match pool.as_slice() {
        [] => Pick::None,
        [only] => Pick::One(only.clone()),
        [first, second, ..] if first.path_rank.is_some() && first.path_rank != second.path_rank => {
            Pick::One(first.clone())
        }
        _ => Pick::Ambiguous(pool.iter().map(qualified).collect()),
    }
}

fn qualified(candidate: &Candidate) -> String {
    format!("{}.{}", candidate.schema, candidate.name)
}

async fn unknown_table(catalog: &Catalog<'_>, requested: &str, name: &str) -> ToolError {
    let sql = format!(
        "SELECT n.nspname::text, c.relname::text \
         FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE {RELATION_KINDS} AND {USER_SCHEMAS} AND NOT c.relispartition \
         ORDER BY 1, 2"
    );
    let known: Vec<(String, String)> = match catalog.fetch(&sql, SUGGESTION_POOL).await {
        Ok(output) => output
            .rows
            .iter()
            .map(|row| {
                (
                    text(&row[0]).unwrap_or_default(),
                    text(&row[1]).unwrap_or_default(),
                )
            })
            .collect(),
        Err(error) => return error,
    };
    let suggestions = closest_names(name, &known);
    let hint = if suggestions.is_empty() {
        "use search_schema or list_tables to find the table".to_string()
    } else {
        format!("closest names: {}", suggestions.join(", "))
    };
    ToolError::new(
        ErrorCategory::UnknownTable,
        "unknown_table",
        format!("no table, view or materialized view named {requested}"),
    )
    .with_hint(hint)
}

fn closest_names(name: &str, known: &[(String, String)]) -> Vec<String> {
    let wanted = name.to_lowercase();
    let wanted_len = wanted.chars().count();
    let tolerance = (wanted_len / 3).max(2);
    let mut scored: Vec<(usize, usize, String)> = known
        .iter()
        .filter_map(|(schema, table)| {
            let candidate = table.to_lowercase();
            let length_gap = candidate.chars().count().abs_diff(wanted_len);
            let score = if !wanted.is_empty()
                && (candidate.contains(&wanted) || wanted.contains(&candidate))
            {
                0
            } else {
                let distance = edit_distance(&wanted, &candidate);
                if distance > tolerance {
                    return None;
                }
                distance
            };
            Some((score, length_gap, format!("{schema}.{table}")))
        })
        .collect();
    scored.sort();
    scored
        .into_iter()
        .take(MAX_SUGGESTIONS)
        .map(|(_, _, name)| name)
        .collect()
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    for (row, left_char) in left.chars().enumerate() {
        let mut current = vec![row + 1; right.len() + 1];
        for (column, right_char) in right.iter().enumerate() {
            let substitution = previous[column] + usize::from(left_char != *right_char);
            current[column + 1] = substitution
                .min(previous[column + 1] + 1)
                .min(current[column] + 1);
        }
        previous = current;
    }
    previous[right.len()]
}

fn describe_sql(oid: i64) -> String {
    let names_of = |array: &str, relation: &str| {
        format!(
            "(SELECT coalesce(json_agg(a.attname ORDER BY k.ord), '[]'::json) \
              FROM unnest({array}) WITH ORDINALITY AS k(attnum, ord) \
              JOIN pg_attribute a ON a.attrelid = {relation} AND a.attnum = k.attnum)"
        )
    };
    let own_columns = names_of("con.conkey", "con.conrelid");
    let referenced_columns = names_of("con.confkey", "con.confrelid");
    format!(
        "SELECT n.nspname::text, c.relname::text, c.relkind::text, c.reltuples::float8, \
           obj_description(c.oid, 'pg_class'), \
           (SELECT json_agg(json_build_object('name', a.attname, 'type', format_type(a.atttypid, a.atttypmod), \
                'not_null', a.attnotnull, 'comment', col_description(c.oid, a.attnum)) ORDER BY a.attnum) \
            FROM pg_attribute a WHERE a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped), \
           (SELECT {own_columns} FROM pg_constraint con WHERE con.conrelid = c.oid AND con.contype = 'p'), \
           (SELECT json_agg(json_build_object('columns', {own_columns}, 'ref_schema', rn.nspname, \
                'ref_table', rc.relname, 'ref_columns', {referenced_columns}) ORDER BY con.conname) \
            FROM pg_constraint con JOIN pg_class rc ON rc.oid = con.confrelid \
            JOIN pg_namespace rn ON rn.oid = rc.relnamespace \
            WHERE con.conrelid = c.oid AND con.contype = 'f'), \
           (SELECT json_agg(json_build_object('schema', sn.nspname, 'table', sc.relname, 'columns', {own_columns}) \
                ORDER BY sn.nspname, sc.relname, con.conname) \
            FROM pg_constraint con JOIN pg_class sc ON sc.oid = con.conrelid \
            JOIN pg_namespace sn ON sn.oid = sc.relnamespace \
            WHERE con.confrelid = c.oid AND con.contype = 'f'), \
           (SELECT json_agg(json_build_object('name', ic.relname, 'unique', i.indisunique, \
                'partial', i.indpred IS NOT NULL, 'columns', \
                (SELECT coalesce(json_agg(a.attname ORDER BY k.ord), '[]'::json) \
                 FROM unnest(i.indkey::int2[]) WITH ORDINALITY AS k(attnum, ord) \
                 LEFT JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = k.attnum \
                 WHERE k.ord <= i.indnkeyatts)) ORDER BY ic.relname) \
            FROM pg_index i JOIN pg_class ic ON ic.oid = i.indexrelid WHERE i.indrelid = c.oid) \
         FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE c.oid = {oid}::oid"
    )
}

fn column_entry(anonymizer: &Anonymizer, column: RawColumn) -> ColumnEntry {
    let type_name = short_type(&column.type_name);
    ColumnEntry {
        pseudonymized: pseudonymization(&column.name, &type_name),
        name: column.name,
        type_name,
        not_null: column.not_null,
        comment: column
            .comment
            .map(|comment| scrub_comment(anonymizer, &comment)),
    }
}

fn pseudonymization(name: &str, type_name: &str) -> Option<&'static str> {
    let class = classify_column(&ColumnInfo {
        column_name: Some(name),
        type_name,
        derived: false,
    });
    if let Some(kind) = class.forced_kind {
        return Some(kind.as_str());
    }
    if class.clear_by_type {
        return None;
    }
    let base = type_name.trim_end_matches("[]");
    match base {
        "inet" | "cidr" => Some("ip_address"),
        "bytea" => Some("withheld"),
        _ => Some(DETECTED_PER_VALUE),
    }
}

fn scrub_comment(anonymizer: &Anonymizer, comment: &str) -> String {
    let scrubbed = anonymizer.free_text(comment);
    let length = scrubbed.chars().count();
    if length <= MAX_COMMENT_CHARS {
        return scrubbed;
    }
    let kept: String = scrubbed.chars().take(MAX_COMMENT_CHARS).collect();
    format!("{kept}…[+{} chars]", length - MAX_COMMENT_CHARS)
}

fn short_type(type_name: &str) -> String {
    const SPELLINGS: [(&str, &str); 7] = [
        ("timestamp with time zone", "timestamptz"),
        ("timestamp without time zone", "timestamp"),
        ("time with time zone", "timetz"),
        ("time without time zone", "time"),
        ("character varying", "varchar"),
        ("double precision", "float8"),
        ("character", "char"),
    ];
    let mut short = type_name.to_string();
    for (long, replacement) in SPELLINGS {
        short = short.replace(long, replacement);
    }
    short
}

fn relation_kind(relkind: &str) -> &'static str {
    match relkind {
        "r" => "table",
        "v" => "view",
        "m" => "materialized_view",
        "p" => "partitioned_table",
        "f" => "foreign_table",
        _ => "other",
    }
}

fn estimate(kind: &str, reltuples: Option<f64>) -> Option<i64> {
    match (kind, reltuples) {
        ("view" | "foreign_table", _) => None,
        (_, Some(value)) if value >= 0.0 => Some(value.round() as i64),
        _ => None,
    }
}

fn split_qualified(input: &str) -> (Option<String>, String) {
    let mut parts = vec![String::new()];
    let mut quoted = false;
    let mut chars = input.trim().chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                if let Some(part) = parts.last_mut() {
                    part.push('"');
                }
            }
            '"' => quoted = !quoted,
            '.' if !quoted => parts.push(String::new()),
            other => {
                if let Some(part) = parts.last_mut() {
                    part.push(other);
                }
            }
        }
    }
    let name = parts.pop().unwrap_or_default();
    let schema = parts.pop().filter(|schema| !schema.is_empty());
    (schema, name)
}

fn literal(text: &str) -> String {
    let escaped = text
        .replace('\0', "")
        .replace('\\', "\\\\")
        .replace('\'', "''");
    format!("E'{escaped}'")
}

fn text(cell: &Cell) -> Option<String> {
    match cell {
        Cell::Text(value) => Some(value.clone()),
        _ => None,
    }
}

fn int(cell: &Cell) -> Option<i64> {
    match cell {
        Cell::Int(value) => Some(*value),
        _ => None,
    }
}

fn float(cell: &Cell) -> Option<f64> {
    match cell {
        Cell::Float(value) => Some(*value),
        Cell::Int(value) => Some(*value as f64),
        _ => None,
    }
}

fn json_cell<T: for<'de> Deserialize<'de>>(cell: &Cell) -> Vec<T> {
    match cell {
        Cell::Json(value) => serde_json::from_value(value.clone()).unwrap_or_default(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(schema: &str, name: &str, path_rank: Option<i64>) -> Candidate {
        Candidate {
            oid: 1,
            schema: schema.into(),
            name: name.into(),
            path_rank,
        }
    }

    #[test]
    fn literals_cannot_escape_their_quotes() {
        assert_eq!(literal("it's"), "E'it''s'");
        assert_eq!(literal("a\\'b"), "E'a\\\\''b'");
        assert_eq!(literal("x\0y"), "E'xy'");
    }

    #[test]
    fn qualified_names_split_on_unquoted_dots() {
        assert_eq!(
            split_qualified("public.orders"),
            (Some("public".into()), "orders".into())
        );
        assert_eq!(split_qualified("orders"), (None, "orders".into()));
        assert_eq!(
            split_qualified("\"My.Schema\".\"Odd\"\"Name\""),
            (Some("My.Schema".into()), "Odd\"Name".into())
        );
        assert_eq!(
            split_qualified("db.sales.orders"),
            (Some("sales".into()), "orders".into())
        );
    }

    #[test]
    fn search_path_order_resolves_bare_names() {
        let picked = pick(
            vec![
                candidate("sales", "orders", Some(2)),
                candidate("public", "orders", Some(1)),
            ],
            None,
            "orders",
        );
        assert_eq!(picked, Pick::One(candidate("public", "orders", Some(1))));
    }

    #[test]
    fn names_outside_the_search_path_are_ambiguous() {
        let picked = pick(
            vec![
                candidate("sales", "orders", None),
                candidate("archive", "orders", None),
            ],
            None,
            "orders",
        );
        assert_eq!(
            picked,
            Pick::Ambiguous(vec!["archive.orders".into(), "sales.orders".into()])
        );
    }

    #[test]
    fn exact_case_wins_over_case_insensitive_matches() {
        let picked = pick(
            vec![
                candidate("public", "Orders", Some(1)),
                candidate("public", "orders", Some(1)),
            ],
            None,
            "Orders",
        );
        assert_eq!(picked, Pick::One(candidate("public", "Orders", Some(1))));
    }

    #[test]
    fn suggestions_prefer_substrings_then_small_edits() {
        let known = vec![
            ("public".to_string(), "customers".to_string()),
            ("public".to_string(), "orders".to_string()),
            ("public".to_string(), "order_lines".to_string()),
            ("public".to_string(), "notes".to_string()),
        ];
        assert_eq!(
            closest_names("order", &known),
            vec!["public.orders", "public.order_lines"]
        );
        assert_eq!(closest_names("custmers", &known), vec!["public.customers"]);
        assert!(closest_names("zzz", &known).is_empty());
    }

    #[test]
    fn pseudonymization_follows_the_query_classifier() {
        assert_eq!(pseudonymization("email", "text"), Some("email"));
        assert_eq!(pseudonymization("id", "integer"), None);
        assert_eq!(pseudonymization("created_at", "timestamptz"), None);
        assert_eq!(
            pseudonymization("label", "varchar(40)"),
            Some(DETECTED_PER_VALUE)
        );
        assert_eq!(pseudonymization("origin", "inet"), Some("ip_address"));
        assert_eq!(pseudonymization("flags", "integer[]"), None);
    }

    #[test]
    fn long_type_spellings_are_shortened() {
        assert_eq!(short_type("timestamp with time zone"), "timestamptz");
        assert_eq!(short_type("character varying(255)[]"), "varchar(255)[]");
        assert_eq!(short_type("character(2)"), "char(2)");
        assert_eq!(short_type("numeric(12,4)"), "numeric(12,4)");
    }

    #[test]
    fn unknown_or_view_estimates_are_null() {
        assert_eq!(estimate("table", Some(-1.0)), None);
        assert_eq!(estimate("table", Some(0.0)), Some(0));
        assert_eq!(estimate("table", Some(9_999.6)), Some(10_000));
        assert_eq!(estimate("view", Some(0.0)), None);
    }

    #[test]
    fn a_sixty_column_description_stays_under_four_kilobytes() {
        let types = [
            "integer",
            "text",
            "varchar(80)",
            "timestamptz",
            "numeric(12,2)",
            "boolean",
        ];
        let columns = (0..60)
            .map(|index| ColumnEntry {
                name: format!("column_nb_{index:02}"),
                type_name: types[index % types.len()].to_string(),
                not_null: index % 3 == 0,
                pseudonymized: (index % types.len() == 1).then_some(DETECTED_PER_VALUE),
                comment: None,
            })
            .collect();
        let response = DescribeResponse {
            schema: "public".into(),
            name: "wide_table".into(),
            kind: "table",
            estimated_rows: Some(120_000),
            comment: None,
            columns,
            primary_key: vec!["column_nb_00".into()],
            foreign_keys: vec![ForeignKey {
                columns: vec!["column_nb_02".into()],
                references: "public.customers(id)".into(),
            }],
            referenced_by: Vec::new(),
            indexes: vec![IndexEntry {
                name: "wide_table_pkey".into(),
                columns: vec!["column_nb_00".into()],
                unique: true,
                partial: false,
            }],
        };
        let size = serde_json::to_string(&response).unwrap().len();
        assert!(size < 4 * 1024, "{size} bytes");
    }
}
