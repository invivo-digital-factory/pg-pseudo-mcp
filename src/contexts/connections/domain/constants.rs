use std::time::Duration;

pub const LOCK_TIMEOUT_MS: u64 = 2_000;
pub const MAX_STATEMENT_TIMEOUT_MS: u64 = i32::MAX as u64;
pub const CONNECT_DEADLINE: Duration = Duration::from_secs(12);
