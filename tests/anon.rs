use pg_pseudo_mcp::anon::{
    ColumnClass, ColumnInfo, Reason, Span, TextVerdict, classify_column, detect_spans,
    dictionary_kind, judge_text, numeric_shape_kind,
};
use pg_pseudo_mcp::kind::Kind;

mod common;

fn direct_text(name: &str) -> ColumnClass {
    classify_column(&ColumnInfo {
        column_name: Some(name),
        type_name: "text",
        derived: false,
    })
}

fn derived_text(name: &str) -> ColumnClass {
    classify_column(&ColumnInfo {
        column_name: Some(name),
        type_name: "text",
        derived: true,
    })
}

fn kinds(spans: &[Span]) -> Vec<Kind> {
    spans.iter().map(|span| span.kind).collect()
}

fn slices<'a>(text: &'a str, spans: &[Span]) -> Vec<&'a str> {
    spans
        .iter()
        .map(|span| &text[span.start..span.end])
        .collect()
}

#[test]
fn personal_column_names_hit_the_dictionary() {
    for (name, kind) in [
        ("email", Kind::Email),
        ("adresse_email", Kind::Email),
        ("courriel", Kind::Email),
        ("telephonePortable", Kind::Phone),
        ("telephone_secours", Kind::Phone),
        ("nom", Kind::LastName),
        ("prenom", Kind::FirstName),
        ("date_naissance", Kind::BirthDate),
        ("iban", Kind::Iban),
        ("siret", Kind::Siret),
        ("siren", Kind::Siren),
        ("nir", Kind::Nir),
        ("ville", Kind::City),
        ("code_postal", Kind::PostalCode),
        ("Prénom", Kind::FirstName),
        ("customerEmail", Kind::Email),
        ("IPAddress", Kind::IpAddress),
    ] {
        assert_eq!(dictionary_kind(name), Some(kind), "{name}");
    }
}

#[test]
fn technical_column_names_miss_the_dictionary() {
    for name in [
        "id",
        "customer_id",
        "status",
        "status_label",
        "created_at",
        "amount",
        "placed_at",
        "titulaire",
        "nombre",
        "connaissance",
        "capacity",
        "adresse_mac",
    ] {
        assert_eq!(dictionary_kind(name), None, "{name}");
    }
}

#[test]
fn clear_types_pass_unless_the_dictionary_forces_them() {
    let class = |name: &str, type_name: &str| {
        classify_column(&ColumnInfo {
            column_name: Some(name),
            type_name,
            derived: false,
        })
    };
    assert!(class("amount", "numeric").clear_by_type);
    assert!(class("id", "int4").clear_by_type);
    assert!(class("created_at", "timestamptz").clear_by_type);
    assert!(class("status", "enum").clear_by_type);
    assert!(class("email_verified", "bool").clear_by_type);
    assert!(class("ref", "uuid").clear_by_type);

    let phone = class("telephone_secours", "int8");
    assert!(!phone.clear_by_type);
    assert_eq!(phone.forced_kind, Some(Kind::Phone));

    let birth = class("date_naissance", "date");
    assert!(!birth.clear_by_type);
    assert_eq!(birth.forced_kind, Some(Kind::BirthDate));

    let sent = class("email_sent_at", "timestamptz");
    assert!(sent.clear_by_type);
    assert_eq!(sent.forced_kind, None);

    let label = class("status_label", "text");
    assert!(!label.clear_by_type);
    assert_eq!(label.forced_kind, None);
}

#[test]
fn free_text_yields_name_phone_and_address_spans() {
    let text = "Appel de M. Durand au 06 12 34 56 78, livraison 12 rue des Lilas";
    let spans = detect_spans(text);
    assert_eq!(
        slices(text, &spans),
        vec!["Durand", "06 12 34 56 78", "12 rue des Lilas"]
    );
    assert_eq!(
        kinds(&spans),
        vec![Kind::LastName, Kind::Phone, Kind::Address]
    );
}

#[test]
fn civility_context_catches_names_absent_from_the_gazetteer() {
    let text = "Appel de M. Brizolier au 06 52 18 74 30, livraison 12 rue des Lilas";
    let spans = detect_spans(text);
    assert_eq!(
        slices(text, &spans),
        vec!["Brizolier", "06 52 18 74 30", "12 rue des Lilas"]
    );
}

#[test]
fn span_offsets_slice_accented_text() {
    let text = "Appel de Mme Lefèvre au 06.12.34.56.78 à Plouzané";
    let spans = detect_spans(text);
    assert_eq!(slices(text, &spans), vec!["Lefèvre", "06.12.34.56.78"]);
}

#[test]
fn iban_is_detected_only_with_a_valid_key() {
    let valid = "virement FR76 3000 6000 0112 3456 7890 189 reçu";
    let spans = detect_spans(valid);
    assert_eq!(
        slices(valid, &spans),
        vec!["FR76 3000 6000 0112 3456 7890 189"]
    );
    assert_eq!(kinds(&spans), vec![Kind::Iban]);

    assert!(detect_spans("virement FR76 3000 6000 0112 3456 7890 188 reçu").is_empty());
    assert_eq!(
        kinds(&detect_spans("FR14 2004 1010 0505 0001 3M02 606")),
        vec![Kind::Iban]
    );
    assert_eq!(
        kinds(&detect_spans("FR7630006000011234567890189")),
        vec![Kind::Iban]
    );
}

#[test]
fn siren_siret_and_nir_need_a_valid_checksum() {
    assert_eq!(kinds(&detect_spans("SIREN 732 829 320")), vec![Kind::Siren]);
    assert!(detect_spans("SIREN 732 829 321").is_empty());
    assert_eq!(
        kinds(&detect_spans("SIRET 732 829 320 00074")),
        vec![Kind::Siret]
    );
    assert_eq!(kinds(&detect_spans("2851175123456 22")), vec![Kind::Nir]);
    assert!(detect_spans("2851175123456 23").is_empty());
}

#[test]
fn emails_cards_and_ip_addresses_are_detected() {
    assert_eq!(kinds(&detect_spans("jean.dupont@x.fr")), vec![Kind::Email]);
    assert_eq!(
        kinds(&detect_spans("carte 4970 1012 3456 7810")),
        vec![Kind::CardNumber]
    );
    assert_eq!(
        kinds(&detect_spans("depuis 192.168.10.4 hier")),
        vec![Kind::IpAddress]
    );
    assert_eq!(
        kinds(&detect_spans("depuis 2001:db8::8a2e:370:7334 hier")),
        vec![Kind::IpAddress]
    );
    assert!(detect_spans("version 999.1.2.3").is_empty());
}

#[test]
fn gazetteer_matches_ignore_case_and_accents() {
    assert_eq!(kinds(&detect_spans("DUPONT")), vec![Kind::LastName]);
    assert_eq!(kinds(&detect_spans("Dupont")), vec![Kind::LastName]);
    assert_eq!(kinds(&detect_spans("Jean Dupont")), vec![Kind::PersonName]);
}

#[test]
fn codes_and_dates_produce_no_spans_and_stay_clear() {
    let class = direct_text("status_label");
    for value in ["PAID", "pending", "FR", "2026-11-19", "en_cours", "", "   "] {
        assert!(detect_spans(value).is_empty(), "{value}");
        assert!(
            matches!(judge_text(value, &class), TextVerdict::Clear),
            "{value}"
        );
    }
}

#[test]
fn bigint_phone_in_a_hinted_column_is_a_phone() {
    let hint = dictionary_kind("telephone_secours");
    assert_eq!(numeric_shape_kind("33641278395", hint), Some(Kind::Phone));
    assert_eq!(numeric_shape_kind("641278395", hint), Some(Kind::Phone));
    assert_eq!(numeric_shape_kind("33641278395", None), None);
    assert_eq!(
        numeric_shape_kind("732829320", Some(Kind::Siren)),
        Some(Kind::Siren)
    );
    assert_eq!(numeric_shape_kind("732829321", Some(Kind::Siren)), None);
}

#[test]
fn short_capitalised_values_in_unknown_columns_default_to_names() {
    let class = direct_text("titulaire");
    assert!(matches!(
        judge_text("Kervadec", &class),
        TextVerdict::WholeValue {
            kind: Kind::LastName,
            reason: Reason::Default
        }
    ));
    assert!(matches!(
        judge_text("Gwenola Kervadec", &class),
        TextVerdict::WholeValue {
            kind: Kind::PersonName,
            reason: Reason::Default
        }
    ));
    assert!(matches!(
        judge_text("Ilona Kervadec", &class),
        TextVerdict::WholeValue {
            kind: Kind::PersonName,
            reason: Reason::Detected
        }
    ));
    assert!(matches!(
        judge_text("Jean-Luc Le Gall", &class),
        TextVerdict::WholeValue {
            kind: Kind::PersonName,
            ..
        }
    ));
    assert!(matches!(judge_text("PAID", &class), TextVerdict::Clear));
    assert!(matches!(judge_text("en_cours", &class), TextVerdict::Clear));
}

#[test]
fn dictionary_columns_replace_the_whole_value() {
    assert!(matches!(
        judge_text("anything", &direct_text("nom")),
        TextVerdict::WholeValue {
            kind: Kind::LastName,
            reason: Reason::Dictionary
        }
    ));
}

#[test]
fn hex_digests_are_opaque_only_in_derived_columns() {
    let digest = "5d41402abc4b2a76b9719d911017c592";
    assert!(matches!(
        judge_text(digest, &derived_text("md5")),
        TextVerdict::Digest
    ));
    assert!(matches!(
        judge_text(digest, &direct_text("checksum")),
        TextVerdict::Clear
    ));
    assert!(matches!(
        judge_text("aGVsbG8gd29ybGQgZm9vYmFy", &derived_text("encode")),
        TextVerdict::Digest
    ));
}

#[test]
fn free_text_in_an_unknown_column_reports_detected_spans() {
    let verdict = judge_text("Rappeler au 06 12 34 56 78", &direct_text("body"));
    match verdict {
        TextVerdict::Spans(spans) => assert_eq!(kinds(&spans), vec![Kind::Phone]),
        _ => panic!("expected spans"),
    }
}

#[test]
fn every_canary_value_is_judged_personal_in_an_unknown_text_column() {
    let class = direct_text("body");
    for value in common::CANARY_VALUES {
        assert!(
            !matches!(judge_text(value, &class), TextVerdict::Clear),
            "{value}"
        );
    }
}

#[test]
fn free_text_keeps_the_text_around_the_spans() {
    let text = "Contact: Ilona Kervadec <ilona.kervadec@canary-mail.fr>";
    let spans = detect_spans(text);
    assert_eq!(
        slices(text, &spans),
        vec!["Ilona Kervadec", "ilona.kervadec@canary-mail.fr"]
    );
    assert_eq!(kinds(&spans), vec![Kind::PersonName, Kind::Email]);
}

#[test]
fn reasons_serialise_in_snake_case() {
    assert_eq!(
        serde_json::to_string(&Reason::Detected).unwrap(),
        "\"detected\""
    );
    assert_eq!(
        serde_json::to_string(&Reason::Derived).unwrap(),
        "\"derived\""
    );
}
