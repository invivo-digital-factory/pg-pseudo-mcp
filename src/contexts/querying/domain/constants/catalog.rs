pub const MAX_PLAN_LINES: usize = 500;
pub const SEARCH_LIMIT: usize = 100;
pub const DEFAULT_LIST_LIMIT: u32 = 100;
pub const MAX_LIST_LIMIT: u32 = 500;
pub const SUGGESTION_POOL: usize = 10_000;
pub const MAX_SUGGESTIONS: usize = 5;
pub const MAX_COMMENT_CHARS: usize = 200;
pub const RELATION_KINDS: &str = "c.relkind IN ('r', 'v', 'm', 'p', 'f')";
pub const USER_SCHEMAS: &str =
    "n.nspname NOT IN ('pg_catalog', 'information_schema') AND n.nspname !~ '^pg_(toast|temp_)'";
pub const DETECTED_PER_VALUE: &str = "detected_per_value";
