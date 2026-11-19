use serde::Serialize;

use crate::limits;

#[derive(Debug, Serialize)]
pub struct ServerInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub engines: &'static [&'static str],
    pub defaults: Limits,
    pub maxima: Limits,
    pub max_cell_chars: usize,
    pub allowed_sql: &'static str,
    pub reversible_kinds: &'static [&'static str],
    pub non_reversible_kinds: &'static [&'static str],
    pub acts_on_real_values: &'static [&'static str],
    pub known_limits: &'static [&'static str],
}

#[derive(Debug, Serialize)]
pub struct Limits {
    pub max_rows: usize,
    pub max_bytes: usize,
    pub timeout_ms: u64,
}

pub fn server_info() -> ServerInfo {
    ServerInfo {
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        engines: &["postgresql"],
        defaults: Limits {
            max_rows: limits::DEFAULT_MAX_ROWS,
            max_bytes: limits::DEFAULT_MAX_BYTES,
            timeout_ms: limits::DEFAULT_TIMEOUT_MS,
        },
        maxima: Limits {
            max_rows: limits::MAX_ROWS_CEILING,
            max_bytes: limits::MAX_BYTES_CEILING,
            timeout_ms: limits::MAX_TIMEOUT_MS,
        },
        max_cell_chars: limits::MAX_CELL_CHARS,
        allowed_sql: "one SELECT, WITH ... SELECT, VALUES or EXPLAIN (without ANALYZE) statement per call",
        reversible_kinds: REVERSIBLE_KINDS,
        non_reversible_kinds: NON_REVERSIBLE_KINDS,
        acts_on_real_values: ACTS_ON_REAL_VALUES,
        known_limits: KNOWN_LIMITS,
    }
}

pub const REVERSIBLE_KINDS: &[&str] = &[
    "email",
    "phone",
    "iban",
    "siren",
    "siret",
    "nir",
    "person_name",
];

pub const NON_REVERSIBLE_KINDS: &[&str] = &[
    "first_name",
    "city",
    "postal_code",
    "civility",
    "gender",
    "birth_date",
];

pub const ACTS_ON_REAL_VALUES: &[&str] = &[
    "ORDER BY, MIN and MAX",
    "range comparisons",
    "LIKE, ILIKE and regular expressions",
    "length and substring functions",
    "hashing and encoding functions",
    "date arithmetic",
];

pub const KNOWN_LIMITS: &[&str] = &[
    "protection targets accidental disclosure by a cooperative agent",
    "character-by-character probing through SQL predicates is not prevented",
    "names absent from the built-in lists inside long free text may pass",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_the_same_limits_as_the_limits_module() {
        let info = server_info();
        assert_eq!(info.defaults.max_rows, limits::DEFAULT_MAX_ROWS);
        assert_eq!(info.maxima.max_rows, limits::MAX_ROWS_CEILING);
        assert_eq!(info.maxima.max_bytes, limits::MAX_BYTES_CEILING);
    }
}
