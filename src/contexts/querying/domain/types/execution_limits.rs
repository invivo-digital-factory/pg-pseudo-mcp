#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionLimits {
    pub max_rows: usize,
    pub timeout_ms: u64,
}
