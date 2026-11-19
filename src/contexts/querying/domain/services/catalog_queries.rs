use super::sql_literal::literal;
use crate::contexts::querying::domain::constants::catalog::{RELATION_KINDS, USER_SCHEMAS};

pub fn search_sql(pattern: &str) -> String {
    let pattern = literal(pattern);
    format!(
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
    )
}

pub fn list_sql(schema: Option<&str>, name_like: Option<&str>, offset: u32) -> String {
    let mut filters = String::new();
    if let Some(schema) = schema {
        filters.push_str(&format!(" AND n.nspname::text = {}", literal(schema)));
    }
    if let Some(name) = name_like {
        filters.push_str(&format!(
            " AND strpos(lower(c.relname::text), lower({})) > 0",
            literal(name)
        ));
    }
    format!(
        "SELECT n.nspname::text, c.relname::text, c.relkind::text, c.reltuples::float8, \
           (SELECT count(*) FROM pg_attribute a WHERE a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped)::int8 \
         FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE {RELATION_KINDS} AND {USER_SCHEMAS} AND NOT c.relispartition{filters} \
         ORDER BY 1, 2 OFFSET {offset}"
    )
}

pub fn resolve_sql(schema: Option<&str>, name: &str) -> String {
    let schema_filter = schema
        .map(|schema| format!(" AND lower(n.nspname::text) = lower({})", literal(schema)))
        .unwrap_or_default();
    format!(
        "SELECT c.oid::int8, n.nspname::text, c.relname::text, \
           array_position(current_schemas(false), n.nspname)::int8 \
         FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE {RELATION_KINDS} AND {USER_SCHEMAS} \
           AND lower(c.relname::text) = lower({}){schema_filter}",
        literal(name)
    )
}

pub fn known_tables_sql() -> String {
    format!(
        "SELECT n.nspname::text, c.relname::text \
         FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE {RELATION_KINDS} AND {USER_SCHEMAS} AND NOT c.relispartition \
         ORDER BY 1, 2"
    )
}
