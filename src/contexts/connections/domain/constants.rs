use std::time::Duration;

pub const LOCK_TIMEOUT_MS: u64 = 2_000;
pub const MAX_STATEMENT_TIMEOUT_MS: u64 = i32::MAX as u64;
pub const CONNECT_DEADLINE: Duration = Duration::from_secs(12);

pub const ENGINE_POSTGRESQL: &str = "postgresql";
pub const ENGINE_MONGODB: &str = "mongodb";
pub const MONGODB_NEEDS_DOCUMENT_TOOLS: &str =
    "this connection is of engine mongodb; use the find or aggregate tool";
pub const MONGODB_DEFAULT_PORT: u16 = 27017;
pub const NOT_A_MONGODB_CONNECTION: &str =
    "this connection is not of engine mongodb; use the query tool";
pub const MONGODB_DATABASE_REQUIRED: &str =
    "this connection declares no default database; name the database";
pub const AGGREGATE_WRITE_STAGE_REFUSED: &str =
    "the pipeline writes with $out or $merge; only reading stages are allowed";
