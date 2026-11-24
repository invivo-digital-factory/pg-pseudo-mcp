use std::time::Duration;

pub const LOCK_TIMEOUT_MS: u64 = 2_000;
pub const MAX_STATEMENT_TIMEOUT_MS: u64 = i32::MAX as u64;
pub const CONNECT_DEADLINE: Duration = Duration::from_secs(12);

pub const ENGINE_POSTGRESQL: &str = "postgresql";
pub const ENGINE_MONGODB: &str = "mongodb";
pub const MONGODB_NOT_SUPPORTED: &str =
    "connections of engine mongodb are not supported by this tool yet";
