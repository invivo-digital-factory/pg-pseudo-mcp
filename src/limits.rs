use serde::Serialize;

pub const DEFAULT_MAX_ROWS: usize = 50;
pub const MAX_ROWS_CEILING: usize = 500;
pub const DEFAULT_MAX_BYTES: usize = 16 * 1024;
pub const MAX_BYTES_CEILING: usize = 64 * 1024;
pub const MIN_BYTES: usize = 1024;
pub const MAX_CELL_CHARS: usize = 500;
pub const DEFAULT_TIMEOUT_MS: u64 = 10_000;
pub const MAX_TIMEOUT_MS: u64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct QueryLimits {
    pub max_rows: usize,
    pub max_bytes: usize,
    pub max_cell_chars: usize,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClampWarning {
    pub code: &'static str,
    pub requested: u64,
    pub applied: u64,
}

impl Default for QueryLimits {
    fn default() -> Self {
        Self {
            max_rows: DEFAULT_MAX_ROWS,
            max_bytes: DEFAULT_MAX_BYTES,
            max_cell_chars: MAX_CELL_CHARS,
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }
}

impl QueryLimits {
    pub fn from_request(
        max_rows: Option<u64>,
        max_bytes: Option<u64>,
        timeout_ms: Option<u64>,
    ) -> (Self, Vec<ClampWarning>) {
        let mut limits = Self::default();
        let mut warnings = Vec::new();
        if let Some(requested) = max_rows {
            let applied = requested.clamp(1, MAX_ROWS_CEILING as u64);
            if applied != requested {
                warnings.push(ClampWarning {
                    code: "max_rows_clamped",
                    requested,
                    applied,
                });
            }
            limits.max_rows = applied as usize;
        }
        if let Some(requested) = max_bytes {
            let applied = requested.clamp(MIN_BYTES as u64, MAX_BYTES_CEILING as u64);
            if applied != requested {
                warnings.push(ClampWarning {
                    code: "max_bytes_clamped",
                    requested,
                    applied,
                });
            }
            limits.max_bytes = applied as usize;
        }
        if let Some(requested) = timeout_ms {
            let applied = requested.clamp(100, MAX_TIMEOUT_MS);
            if applied != requested {
                warnings.push(ClampWarning {
                    code: "timeout_clamped",
                    requested,
                    applied,
                });
            }
            limits.timeout_ms = applied;
        }
        (limits, warnings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_above_the_ceiling_are_clamped_with_a_warning() {
        let (limits, warnings) = QueryLimits::from_request(Some(900), None, None);
        assert_eq!(limits.max_rows, MAX_ROWS_CEILING);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, "max_rows_clamped");
        assert_eq!(warnings[0].applied, 500);
    }

    #[test]
    fn a_byte_budget_above_the_ceiling_is_clamped_with_a_warning() {
        let (limits, warnings) = QueryLimits::from_request(None, Some(1_000_000), None);
        assert_eq!(limits.max_bytes, MAX_BYTES_CEILING);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, "max_bytes_clamped");
        assert_eq!(warnings[0].applied, 65_536);
        let (limits, warnings) = QueryLimits::from_request(None, Some(0), None);
        assert_eq!(limits.max_bytes, MIN_BYTES);
        assert_eq!(warnings[0].code, "max_bytes_clamped");
    }

    #[test]
    fn requests_within_bounds_are_kept_silently() {
        let (limits, warnings) = QueryLimits::from_request(Some(10), Some(32_768), Some(5_000));
        assert_eq!(limits.max_rows, 10);
        assert_eq!(limits.max_bytes, 32_768);
        assert_eq!(limits.timeout_ms, 5_000);
        assert!(warnings.is_empty());
    }

    #[test]
    fn no_request_yields_defaults() {
        let (limits, warnings) = QueryLimits::from_request(None, None, None);
        assert_eq!(limits, QueryLimits::default());
        assert!(warnings.is_empty());
    }
}
