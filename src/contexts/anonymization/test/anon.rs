use crate::contexts::anonymization::domain::services::classify::{
    classify_column, judge_text, numeric_shape_kind,
};
use crate::contexts::anonymization::domain::services::detect::detect_spans;
use crate::contexts::anonymization::domain::services::dictionary::dictionary_kind;
use crate::contexts::anonymization::domain::types::{
    ColumnClass, ColumnInfo, Kind, Reason, Span, TextVerdict,
};

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

mod unit {
    use super::*;

    #[test]
    fn un_nom_de_colonne_personnel_est_reconnu_par_le_dictionnaire() {
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
    fn un_nom_de_colonne_technique_n_est_pas_reconnu_par_le_dictionnaire() {
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
    fn un_type_neutre_reste_en_clair_sauf_si_le_dictionnaire_l_impose() {
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
    fn un_texte_libre_revele_nom_telephone_et_adresse() {
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
    fn une_civilite_revele_un_nom_absent_du_gazetier() {
        let text = "Appel de M. Brizolier au 06 52 18 74 30, livraison 12 rue des Lilas";
        let spans = detect_spans(text);
        assert_eq!(
            slices(text, &spans),
            vec!["Brizolier", "06 52 18 74 30", "12 rue des Lilas"]
        );
    }

    #[test]
    fn les_positions_des_segments_decoupent_correctement_un_texte_accentue() {
        let text = "Appel de Mme Lefèvre au 06.12.34.56.78 à Plouzané";
        let spans = detect_spans(text);
        assert_eq!(slices(text, &spans), vec!["Lefèvre", "06.12.34.56.78"]);
    }

    #[test]
    fn un_iban_n_est_detecte_qu_avec_une_cle_valide() {
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
    fn siren_siret_et_nir_exigent_une_somme_de_controle_valide() {
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
    fn les_emails_cartes_et_adresses_ip_sont_detectes() {
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
    fn le_gazetier_ignore_la_casse_et_les_accents() {
        assert_eq!(kinds(&detect_spans("DUPONT")), vec![Kind::LastName]);
        assert_eq!(kinds(&detect_spans("Dupont")), vec![Kind::LastName]);
        assert_eq!(kinds(&detect_spans("Jean Dupont")), vec![Kind::PersonName]);
    }

    #[test]
    fn les_codes_et_les_dates_ne_produisent_aucun_segment_et_restent_en_clair() {
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
    fn un_telephone_en_bigint_dans_une_colonne_indicee_est_un_telephone() {
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
    fn une_valeur_courte_capitalisee_en_colonne_inconnue_est_prise_pour_un_nom() {
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
            judge_text("Solène Kervadec", &class),
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
    fn une_colonne_du_dictionnaire_remplace_la_valeur_entiere() {
        assert!(matches!(
            judge_text("anything", &direct_text("nom")),
            TextVerdict::WholeValue {
                kind: Kind::LastName,
                reason: Reason::Dictionary
            }
        ));
    }

    #[test]
    fn une_empreinte_hexadecimale_n_est_opaque_qu_en_colonne_derivee() {
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
    fn un_texte_libre_en_colonne_inconnue_rapporte_les_segments_detectes() {
        let verdict = judge_text("Rappeler au 06 12 34 56 78", &direct_text("body"));
        match verdict {
            TextVerdict::Spans(spans) => assert_eq!(kinds(&spans), vec![Kind::Phone]),
            _ => panic!("expected spans"),
        }
    }

    #[test]
    fn un_texte_libre_conserve_le_texte_autour_des_segments() {
        let text = "Contact: Solène Kervadec <ylwenna.kervadec@canary-mail.fr>";
        let spans = detect_spans(text);
        assert_eq!(
            slices(text, &spans),
            vec!["Solène Kervadec", "ylwenna.kervadec@canary-mail.fr"]
        );
        assert_eq!(kinds(&spans), vec![Kind::PersonName, Kind::Email]);
    }

    #[test]
    fn les_motifs_sont_serialises_en_snake_case() {
        assert_eq!(
            serde_json::to_string(&Reason::Detected).unwrap(),
            "\"detected\""
        );
        assert_eq!(
            serde_json::to_string(&Reason::Derived).unwrap(),
            "\"derived\""
        );
    }
}
