pub const DEFAULT_COST_LIMIT: u64 = 100_000;
pub const DEFAULT_SCAN_LIMIT: u64 = 1_000_000;
pub const PG_SEQ_SCAN: &str = "Seq Scan";
pub const PG_LIMIT: &str = "Limit";
pub const PG_PASS_THROUGH: &[&str] = &["Gather", "Result", "Subquery Scan", "Append"];
pub const MONGO_COLLSCAN: &str = "COLLSCAN";
pub const MONGO_LIMIT: &str = "LIMIT";
pub const MONGO_PASS_THROUGH: &[&str] = &[
    "PROJECTION_SIMPLE",
    "PROJECTION_DEFAULT",
    "PROJECTION_COVERED",
];
pub const MONGO_CHILD_STAGES: &[&str] = &["inputStage", "innerStage", "outerStage"];
pub const CONFIRMATION_HINT: &str = "show this estimate to the developer and ask whether to run the request anyway; only after they approve, repeat the same call with confirm_heavy: true";
pub const CHECK_SKIPPED: &str = "heavy_query_check_skipped";
