use super::detect::{detect_spans, structural_spans};
use super::detect::{french_phone_digits, luhn, nir_valid};
use super::dictionary::dictionary_kind;
use super::freetext;
use crate::contexts::anonymization::domain::types::{
    ColumnClass, ColumnInfo, Kind, Reason, TextVerdict,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeFamily {
    AlwaysClear,
    Numeric,
    Temporal,
    Other,
}

fn type_family(type_name: &str) -> TypeFamily {
    let lower = type_name.trim().to_ascii_lowercase();
    let element = lower
        .strip_suffix("[]")
        .or_else(|| lower.strip_prefix('_'))
        .unwrap_or(&lower);
    let base = element.split('(').next().unwrap_or(element).trim();
    match base {
        "bool" | "boolean" | "uuid" | "interval" | "enum" | "anyenum" => TypeFamily::AlwaysClear,
        "int2" | "int4" | "int8" | "smallint" | "integer" | "int" | "bigint" | "numeric"
        | "decimal" | "real" | "float4" | "float8" | "double precision" | "money" | "oid"
        | "smallserial" | "serial" | "bigserial" | "serial2" | "serial4" | "serial8" => {
            TypeFamily::Numeric
        }
        "date" | "time" | "timetz" => TypeFamily::Temporal,
        _ if base.starts_with("timestamp") || base.starts_with("time ") => TypeFamily::Temporal,
        _ => TypeFamily::Other,
    }
}

pub fn classify_column(info: &ColumnInfo) -> ColumnClass {
    let dictionary = info.column_name.and_then(dictionary_kind);
    let (forced_kind, clear_by_type) = match type_family(info.type_name) {
        TypeFamily::AlwaysClear => (None, true),
        TypeFamily::Numeric => (dictionary, dictionary.is_none()),
        TypeFamily::Temporal if dictionary == Some(Kind::BirthDate) => (dictionary, false),
        TypeFamily::Temporal => (None, true),
        TypeFamily::Other => (dictionary, false),
    };
    ColumnClass {
        forced_kind,
        clear_by_type,
        derived: info.derived,
        structural_only: false,
        forced_by_policy: false,
    }
}

pub fn judge_text(text: &str, class: &ColumnClass) -> TextVerdict {
    let trimmed = text.trim();
    if trimmed.is_empty() || class.clear_by_type {
        return TextVerdict::Clear;
    }
    if let Some(kind) = class.forced_kind {
        return TextVerdict::WholeValue {
            kind,
            reason: forced_reason(class),
        };
    }
    if class.structural_only {
        let spans = structural_spans(text);
        return if spans.is_empty() {
            TextVerdict::Clear
        } else {
            TextVerdict::Spans(spans)
        };
    }
    if class.derived && freetext::is_digest(trimmed) {
        return TextVerdict::Digest;
    }
    let spans = detect_spans(text);
    let shape = freetext::name_shape(trimmed).or_else(|| {
        if class.derived {
            freetext::any_case_name_shape(trimmed)
        } else {
            None
        }
    });
    if let Some(kind) = shape {
        let reason = if spans.is_empty() {
            Reason::Default
        } else {
            Reason::Detected
        };
        return TextVerdict::WholeValue { kind, reason };
    }
    if spans.is_empty() {
        TextVerdict::Clear
    } else {
        TextVerdict::Spans(spans)
    }
}

pub fn forced_reason(class: &ColumnClass) -> Reason {
    if class.forced_by_policy {
        Reason::Policy
    } else {
        Reason::Dictionary
    }
}

fn integral_digits(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    let unsigned = trimmed
        .strip_prefix('+')
        .or_else(|| trimmed.strip_prefix('-'))
        .unwrap_or(trimmed);
    let integral = match unsigned.split_once('.') {
        Some((integral, fraction)) if fraction.bytes().all(|b| b == b'0') => integral,
        Some(_) => return None,
        None => unsigned,
    };
    (!integral.is_empty() && integral.bytes().all(|b| b.is_ascii_digit())).then_some(integral)
}

pub fn numeric_shape_kind(digits: &str, column_hint: Option<Kind>) -> Option<Kind> {
    let hint = column_hint?;
    let digits = integral_digits(digits)?;
    let shaped = match hint {
        Kind::Phone => french_phone_digits(digits),
        Kind::Siren => digits.len() == 9 && luhn(digits),
        Kind::Siret => digits.len() == 14 && luhn(digits),
        Kind::Nir => nir_valid(digits),
        Kind::CardNumber => (13..=19).contains(&digits.len()) && luhn(digits),
        Kind::PostalCode => matches!(digits.len(), 4 | 5),
        _ => false,
    };
    shaped.then_some(hint)
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn les_familles_de_types_couvrent_les_graphies_postgres() {
        assert_eq!(type_family("int8"), TypeFamily::Numeric);
        assert_eq!(type_family("numeric(12,4)"), TypeFamily::Numeric);
        assert_eq!(type_family("_int4"), TypeFamily::Numeric);
        assert_eq!(
            type_family("timestamp with time zone"),
            TypeFamily::Temporal
        );
        assert_eq!(type_family("timestamptz"), TypeFamily::Temporal);
        assert_eq!(type_family("bool"), TypeFamily::AlwaysClear);
        assert_eq!(type_family("text[]"), TypeFamily::Other);
        assert_eq!(type_family("jsonb"), TypeFamily::Other);
    }

    #[test]
    fn la_forme_numerique_lit_les_rendus_decimaux() {
        assert_eq!(
            numeric_shape_kind("33641278395.0000", Some(Kind::Phone)),
            Some(Kind::Phone)
        );
        assert_eq!(numeric_shape_kind("12.5", Some(Kind::Phone)), None);
        assert_eq!(
            numeric_shape_kind("285117512345622", Some(Kind::Nir)),
            Some(Kind::Nir)
        );
    }
}
