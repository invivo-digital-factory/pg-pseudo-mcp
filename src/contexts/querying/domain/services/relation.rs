pub fn short_type(type_name: &str) -> String {
    const SPELLINGS: [(&str, &str); 7] = [
        ("timestamp with time zone", "timestamptz"),
        ("timestamp without time zone", "timestamp"),
        ("time with time zone", "timetz"),
        ("time without time zone", "time"),
        ("character varying", "varchar"),
        ("double precision", "float8"),
        ("character", "char"),
    ];
    let mut short = type_name.to_string();
    for (long, replacement) in SPELLINGS {
        short = short.replace(long, replacement);
    }
    short
}

pub fn relation_kind(relkind: &str) -> &'static str {
    match relkind {
        "r" => "table",
        "v" => "view",
        "m" => "materialized_view",
        "p" => "partitioned_table",
        "f" => "foreign_table",
        _ => "other",
    }
}

pub fn estimate(kind: &str, reltuples: Option<f64>) -> Option<i64> {
    match (kind, reltuples) {
        ("view" | "foreign_table", _) => None,
        (_, Some(value)) if value >= 0.0 => Some(value.round() as i64),
        _ => None,
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn les_noms_de_types_longs_sont_abreges() {
        assert_eq!(short_type("timestamp with time zone"), "timestamptz");
        assert_eq!(short_type("character varying(255)[]"), "varchar(255)[]");
        assert_eq!(short_type("character(2)"), "char(2)");
        assert_eq!(short_type("numeric(12,4)"), "numeric(12,4)");
    }

    #[test]
    fn l_estimation_inconnue_ou_d_une_vue_est_nulle() {
        assert_eq!(estimate("table", Some(-1.0)), None);
        assert_eq!(estimate("table", Some(0.0)), Some(0));
        assert_eq!(estimate("table", Some(9_999.6)), Some(10_000));
        assert_eq!(estimate("view", Some(0.0)), None);
    }
}
