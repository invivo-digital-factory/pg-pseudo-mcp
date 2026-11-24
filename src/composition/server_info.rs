use serde::Serialize;

use crate::contexts::querying::domain::constants::{limits, mongo_policy};

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
    pub mongodb: MongoInfo,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<InfoWarning>,
}

#[derive(Debug, Serialize)]
pub struct MongoInfo {
    pub tools: &'static [&'static str],
    pub stages: &'static [&'static str],
    pub first_stage_only: &'static [&'static str],
    pub query_operators: Vec<&'static str>,
    pub expression_operators: &'static [&'static str],
    pub max_pipeline_stages: usize,
    pub max_nesting_depth: usize,
    pub max_request_bytes: usize,
    pub max_request_array_elements: usize,
    pub max_returned_array_items: usize,
    pub pseudonym_translation: &'static str,
    pub acts_on_real_values: &'static [&'static str],
    pub known_limits: &'static [&'static str],
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
        engines: &["postgresql", "mongodb"],
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
        mongodb: mongo_info(),
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

fn mongo_info() -> MongoInfo {
    MongoInfo {
        tools: &["find", "aggregate"],
        stages: mongo_policy::STAGES,
        first_stage_only: mongo_policy::FIRST_STAGE_ONLY,
        query_operators: mongo_policy::TOP_LEVEL_QUERY_OPERATORS
            .iter()
            .chain(mongo_policy::FIELD_QUERY_OPERATORS)
            .chain(mongo_policy::GEOMETRY_OPERATORS)
            .chain(mongo_policy::TEXT_OPERATORS)
            .copied()
            .collect(),
        expression_operators: mongo_policy::EXPRESSION_OPERATORS,
        max_pipeline_stages: mongo_policy::MAX_PIPELINE_STAGES,
        max_nesting_depth: mongo_policy::MAX_NESTING_DEPTH,
        max_request_bytes: mongo_policy::MAX_REQUEST_BYTES,
        max_request_array_elements: mongo_policy::MAX_ARRAY_ELEMENTS,
        max_returned_array_items: mongo_policy::MAX_RETURNED_ARRAY_ITEMS,
        pseudonym_translation: mongo_policy::NOT_TRANSLATED_POSITIONS,
        acts_on_real_values: MONGO_ACTS_ON_REAL_VALUES,
        known_limits: MONGO_KNOWN_LIMITS,
    }
}

pub const MONGO_ACTS_ON_REAL_VALUES: &[&str] = &[
    "sort, $min and $max",
    "range operators ($gt, $gte, $lt, $lte)",
    "$regex, $text and string operators ($substrCP, $strLenCP, $indexOfCP)",
    "counts and $group keys",
    "date operators",
];

pub const MONGO_KNOWN_LIMITS: &[&str] = &[
    "the guard is the only protection against writes when the MongoDB user holds readWrite",
    "character-by-character probing through $regex or string operators is not prevented",
    "describe_table reads a random sample of up to 100 documents; rare fields may be missing",
    "a field computed by a pipeline is in clear only when every field it reads is declared clear in the column policy",
    "object keys that contain a dot are never matched by the column policy",
];

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
