use serde::Serialize;

use crate::contexts::querying::domain::constants::limits;

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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<InfoWarning>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InfoWarning {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct Limits {
    pub max_rows: usize,
    pub max_bytes: usize,
    pub timeout_ms: u64,
}

pub const COLUMN_POLICY_IGNORED: &str = "column_policy_ignored";
pub const CONFIG_LOCATION_DEPRECATED: &str = "config_location_deprecated";
pub const DATA_DIRECTORY_DEPRECATED: &str = "data_directory_deprecated";
pub const DATA_DIRECTORY_CONFLICT: &str = "data_directory_conflict";

pub fn server_info(
    column_policy_warnings: Vec<String>,
    deprecations: Vec<InfoWarning>,
) -> ServerInfo {
    ServerInfo {
        name: super::server::SERVER_NAME,
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
        warnings: column_policy_warnings
            .into_iter()
            .map(|message| InfoWarning {
                code: COLUMN_POLICY_IGNORED,
                message,
            })
            .chain(deprecations)
            .collect(),
    }
}

pub const REVERSIBLE_KINDS: &[&str] = &[
    "email",
    "phone",
    "iban",
    "siren",
    "siret",
    "nir",
    "card_number",
    "ip_address",
    "person_name",
    "last_name",
];

pub const NON_REVERSIBLE_KINDS: &[&str] = &[
    "first_name",
    "address",
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
    "intervals computed from birth dates (age(), date differences) are exact and reveal the real birth date",
    "free text cut by SQL string functions (left, substr) can return fragments of personal values that no detector recognises",
    "date parts extracted from birth dates (to_char, extract) are returned in clear",
    "a column wrongly declared clear in the column policy is returned in clear, apart from values the structural detectors recognise",
];

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn les_infos_serveur_exposent_les_memes_limites_que_le_module_limits() {
        let info = server_info(Vec::new(), Vec::new());
        assert_eq!(info.defaults.max_rows, limits::DEFAULT_MAX_ROWS);
        assert_eq!(info.maxima.max_rows, limits::MAX_ROWS_CEILING);
        assert_eq!(info.maxima.max_bytes, limits::MAX_BYTES_CEILING);
    }

    #[test]
    fn les_types_annonces_correspondent_a_leur_reversibilite() {
        use crate::contexts::anonymization::domain::types::Kind;
        let mut advertised = 0;
        for (names, reversible) in [(REVERSIBLE_KINDS, true), (NON_REVERSIBLE_KINDS, false)] {
            for name in names {
                let kind = Kind::parse(name).expect("advertised kind exists");
                assert_eq!(kind.is_reversible(), reversible, "{name}");
                advertised += 1;
            }
        }
        assert_eq!(advertised, Kind::ALL.len());
    }
}
