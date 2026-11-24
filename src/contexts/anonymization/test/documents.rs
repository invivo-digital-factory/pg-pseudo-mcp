use super::canary_values::CANARY_VALUES;
use crate::contexts::anonymization::domain::services::anonymizer::Anonymizer;
use crate::contexts::anonymization::domain::services::pseudonymizer::Pseudonymizer;
use crate::contexts::anonymization::domain::types::{
    AnonymizedDocuments, Cell, ColumnFlag, ColumnMeta, ColumnSource, FieldClearance, FieldPath,
    GeneratorConfig, Kind, Node, Reason,
};
use crate::contexts::anonymization::infrastructure::data_sources::data_directory::open_pseudonymizer;

fn pseudonymizer() -> (tempfile::TempDir, Pseudonymizer) {
    let dir = tempfile::tempdir().expect("data dir");
    let pseudonymizer =
        open_pseudonymizer(dir.path(), GeneratorConfig::default()).expect("pseudonymizer");
    (dir, pseudonymizer)
}

fn object(entries: Vec<(&str, Node)>) -> Node {
    Node::Object(
        entries
            .into_iter()
            .map(|(key, node)| (key.to_string(), node))
            .collect(),
    )
}

fn text(value: &str) -> Node {
    Node::Text(value.to_string())
}

fn clearance(path: &str, clearance: FieldClearance) -> (FieldPath, FieldClearance) {
    (FieldPath::new(path), clearance)
}

fn field<'a>(node: &'a Node, path: &str) -> &'a Node {
    path.split('.').fold(node, |current, key| match current {
        Node::Object(entries) => entries
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, child)| child)
            .unwrap_or_else(|| panic!("no field {key} in {current:?}")),
        other => panic!("{other:?} has no field {key}"),
    })
}

fn flag(kind: &str, why: Reason) -> Option<ColumnFlag> {
    Some(ColumnFlag {
        kind: kind.to_string(),
        why,
    })
}

fn flagged(anonymized: &AnonymizedDocuments, path: &str) -> Option<ColumnFlag> {
    anonymized.pseudonymized.get(path).cloned()
}

fn assert_no_canary(anonymized: &AnonymizedDocuments) {
    let response = format!("{:?} {:?}", anonymized.documents, anonymized.pseudonymized);
    for canary in CANARY_VALUES {
        for form in [
            canary.to_string(),
            canary.to_lowercase(),
            canary.to_uppercase(),
        ] {
            assert!(!response.contains(&form), "{form} leaked in {response}");
        }
    }
}

fn customer() -> Node {
    object(vec![
        (
            "_id",
            Node::ObjectId("6650f1c2a4b3e2d1c0b9a8f7".to_string()),
        ),
        ("firstName", text("Ylwenna")),
        ("lastName", text("Kervadec")),
        (
            "contact",
            object(vec![
                ("email", text("ylwenna.kervadec@canary-mail.fr")),
                (
                    "phones",
                    Node::Array(vec![text("06 41 27 83 95"), text("07 58 14 92 36")]),
                ),
            ]),
        ),
        (
            "createdAt",
            Node::Date("2026-03-02T09:15:00.000Z".to_string()),
        ),
        ("status", text("PAID")),
    ])
}

mod integration {
    use super::*;

    #[test]
    fn ae5_un_client_fait_de_valeurs_canari_revient_pseudonymise_aux_quatre_chemins_personnels() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);

        let anonymized = anonymizer.documents(vec![customer()], &[]);

        let document = &anonymized.documents[0];
        assert_eq!(
            anonymized.pseudonymized.keys().collect::<Vec<_>>(),
            ["contact.email", "contact.phones", "firstName", "lastName"]
        );
        assert_eq!(
            flagged(&anonymized, "firstName"),
            flag("first_name", Reason::Dictionary)
        );
        assert_eq!(
            flagged(&anonymized, "lastName"),
            flag("last_name", Reason::Dictionary)
        );
        assert_eq!(
            flagged(&anonymized, "contact.email"),
            flag("email", Reason::Dictionary)
        );
        assert_eq!(
            flagged(&anonymized, "contact.phones"),
            flag("phone", Reason::Dictionary)
        );
        assert_eq!(
            field(document, "contact.email"),
            &text(
                &pseudonymizer
                    .pseudonymize(Kind::Email, "ylwenna.kervadec@canary-mail.fr")
                    .text
            )
        );
        assert_eq!(field(document, "_id"), field(&customer(), "_id"));
        assert_eq!(
            field(document, "createdAt"),
            field(&customer(), "createdAt")
        );
        assert_eq!(field(document, "status"), &text("PAID"));
        assert_eq!(anonymized.omitted_paths, 0);
        assert_no_canary(&anonymized);
    }

    #[test]
    fn une_valeur_courte_capitalisee_dans_un_champ_inconnu_est_pseudonymisee_par_defaut() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![
            ("titulaire", text("Kervadec")),
            ("etat", text("PAID")),
        ]);

        let anonymized = anonymizer.documents(vec![document], &[]);

        assert_ne!(
            field(&anonymized.documents[0], "titulaire"),
            &text("Kervadec")
        );
        assert_eq!(
            flagged(&anonymized, "titulaire"),
            flag("last_name", Reason::Default)
        );
        assert_eq!(field(&anonymized.documents[0], "etat"), &text("PAID"));
        assert_eq!(flagged(&anonymized, "etat"), None);
        assert_no_canary(&anonymized);
    }

    #[test]
    fn une_table_indexee_par_des_adresses_e_mail_voit_ses_cles_remplacees_par_des_pseudonymes() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![(
            "abonnes",
            object(vec![
                ("ylwenna.kervadec@canary-mail.fr", Node::Bool(true)),
                ("o.hazvekmor@canary-mail.fr", Node::Bool(false)),
            ]),
        )]);

        let anonymized = anonymizer.documents(vec![document], &[]);

        let Node::Object(entries) = field(&anonymized.documents[0], "abonnes") else {
            panic!("abonnes stays an object");
        };
        let first = pseudonymizer
            .pseudonymize(Kind::Email, "ylwenna.kervadec@canary-mail.fr")
            .text;
        let second = pseudonymizer
            .pseudonymize(Kind::Email, "o.hazvekmor@canary-mail.fr")
            .text;
        assert_eq!(
            entries,
            &vec![
                (first.clone(), Node::Bool(true)),
                (second, Node::Bool(false))
            ]
        );
        assert_eq!(
            flagged(&anonymized, &format!("abonnes.{first}")),
            flag("email", Reason::Detected)
        );
        assert_no_canary(&anonymized);
    }

    #[test]
    fn une_date_de_naissance_est_decalee_mais_pas_une_date_de_creation() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let birth = "1987-03-14T00:00:00.000Z";
        let created = "2026-03-02T09:15:00.000Z";
        let document = object(vec![
            ("birthDate", Node::Date(birth.to_string())),
            ("createdAt", Node::Date(created.to_string())),
        ]);

        let anonymized = anonymizer.documents(vec![document], &[]);

        let Node::Date(shifted) = field(&anonymized.documents[0], "birthDate") else {
            panic!("a birth date stays a date");
        };
        assert_ne!(shifted, birth);
        assert!(shifted.ends_with("T00:00:00.000Z"), "{shifted}");
        assert_eq!(
            flagged(&anonymized, "birthDate"),
            flag("birth_date", Reason::Dictionary)
        );
        assert_eq!(
            field(&anonymized.documents[0], "createdAt"),
            &Node::Date(created.to_string())
        );
        assert_eq!(flagged(&anonymized, "createdAt"), None);
        assert_no_canary(&anonymized);
    }

    #[test]
    fn un_telephone_en_int64_sous_telephone_est_pseudonymise_mais_pas_une_quantite() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![
            ("telephone", Node::Int(33641278395)),
            ("quantity", Node::Int(33612345678)),
        ]);

        let anonymized = anonymizer.documents(vec![document], &[]);

        let phone = field(&anonymized.documents[0], "telephone");
        assert_ne!(phone, &Node::Int(33641278395));
        assert!(matches!(phone, Node::Int(_) | Node::Text(_)), "{phone:?}");
        assert_eq!(
            flagged(&anonymized, "telephone"),
            flag("phone", Reason::Dictionary)
        );
        assert_eq!(
            field(&anonymized.documents[0], "quantity"),
            &Node::Int(33612345678)
        );
        assert_eq!(flagged(&anonymized, "quantity"), None);
        assert_no_canary(&anonymized);
    }

    #[test]
    fn la_meme_adresse_e_mail_recoit_le_meme_pseudonyme_dans_une_ligne_et_dans_un_document() {
        let (dir, pseudonymizer) = pseudonymizer();
        let email = "ylwenna.kervadec@canary-mail.fr";
        let row = Anonymizer::new(&pseudonymizer, &[]).rows(
            &[ColumnMeta {
                name: "email".to_string(),
                type_name: "text".to_string(),
                source: Some(ColumnSource {
                    schema: "public".to_string(),
                    table: "customers".to_string(),
                    column: "email".to_string(),
                }),
            }],
            vec![vec![Cell::Text(email.to_string())]],
            None,
        );
        drop(pseudonymizer);
        let reopened =
            open_pseudonymizer(dir.path(), GeneratorConfig::default()).expect("pseudonymizer");

        let anonymized = Anonymizer::new(&reopened, &[]).documents(
            vec![object(vec![(
                "customer",
                object(vec![("mail", text(email))]),
            )])],
            &[],
        );

        let from_row = row.rows[0][0].as_str().expect("text").to_string();
        assert_ne!(from_row, email);
        assert_eq!(
            field(&anonymized.documents[0], "customer.mail"),
            &Node::Text(from_row)
        );
        assert_no_canary(&anonymized);
    }

    #[test]
    fn un_champ_declare_personnel_est_pseudonymise_avec_le_type_donne_et_la_raison_politique() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![
            ("code_lot", text("lot-42")),
            ("numero", Node::Int(418273652)),
            ("libre", text("lot-43")),
        ]);
        let clearances = [
            clearance("code_lot", FieldClearance::Personal(Kind::LastName)),
            clearance("NUMERO", FieldClearance::Personal(Kind::Siren)),
        ];

        let anonymized = anonymizer.documents(vec![document], &clearances);

        assert_ne!(field(&anonymized.documents[0], "code_lot"), &text("lot-42"));
        assert_eq!(
            flagged(&anonymized, "code_lot"),
            flag("last_name", Reason::Policy)
        );
        assert_ne!(
            field(&anonymized.documents[0], "numero"),
            &Node::Int(418273652)
        );
        assert_eq!(
            flagged(&anonymized, "numero"),
            flag("siren", Reason::Policy)
        );
        assert_eq!(field(&anonymized.documents[0], "libre"), &text("lot-43"));
        assert_no_canary(&anonymized);
    }

    #[test]
    fn un_champ_declare_en_clair_garde_les_noms_mais_remplace_une_adresse_e_mail() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![
            ("nom", text("Garance Bleue")),
            ("libelle", text("Garance ylwenna.kervadec@canary-mail.fr")),
            ("telephone", Node::Int(33612345678)),
        ]);
        let clearances = [
            clearance("nom", FieldClearance::Clear),
            clearance("libelle", FieldClearance::Clear),
            clearance("telephone", FieldClearance::Clear),
        ];

        let anonymized = anonymizer.documents(vec![document], &clearances);

        let document = &anonymized.documents[0];
        assert_eq!(field(document, "nom"), &text("Garance Bleue"));
        assert_eq!(flagged(&anonymized, "nom"), None);
        let Node::Text(label) = field(document, "libelle") else {
            panic!("libelle stays text");
        };
        assert!(label.starts_with("Garance "), "{label}");
        assert!(!label.contains("canary-mail"), "{label}");
        assert_eq!(
            flagged(&anonymized, "libelle"),
            flag("email", Reason::Detected)
        );
        assert_eq!(field(document, "telephone"), &Node::Int(33612345678));
        assert_no_canary(&anonymized);
    }

    #[test]
    fn au_dela_de_deux_cents_chemins_la_liste_est_tronquee_et_compte_les_chemins_omis() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let wide = |count: usize| {
            Node::Object(
                (0..count)
                    .map(|index| (format!("remarque{index}"), text("Kervadec")))
                    .collect(),
            )
        };

        let anonymized = anonymizer.documents(vec![wide(150), wide(250)], &[]);

        assert_eq!(anonymized.pseudonymized.len(), 200);
        assert_eq!(anonymized.omitted_paths, 50);
        for document in &anonymized.documents {
            let Node::Object(entries) = document else {
                panic!("documents stay objects");
            };
            assert!(entries.iter().all(|(_, value)| value != &text("Kervadec")));
        }
        assert_no_canary(&anonymized);
    }

    #[test]
    fn un_meme_chemin_repete_dans_plusieurs_documents_ne_compte_qu_une_fois() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);

        let anonymized = anonymizer.documents(vec![customer(); 300], &[]);

        assert_eq!(anonymized.pseudonymized.len(), 4);
        assert_eq!(anonymized.omitted_paths, 0);
        assert_eq!(anonymized.documents.len(), 300);
        assert_no_canary(&anonymized);
    }

    #[test]
    fn les_types_bson_neutres_restent_en_clair_meme_sous_un_nom_personnel() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let neutral = vec![
            Node::ObjectId("6650f1c2a4b3e2d1c0b9a8f7".to_string()),
            Node::Bool(true),
            Node::Null,
            Node::Timestamp("1716630000:1".to_string()),
            Node::MinKey,
            Node::MaxKey,
            Node::Uuid {
                subtype: 4,
                value: "3b241101-e2bb-4255-8caf-4136c566a962".to_string(),
            },
        ];
        let document = object(vec![
            ("nom", Node::Array(neutral.clone())),
            ("email", Node::Array(neutral.clone())),
        ]);
        let clearances = [clearance("email", FieldClearance::Personal(Kind::Email))];

        let anonymized = anonymizer.documents(vec![document], &clearances);

        assert_eq!(
            field(&anonymized.documents[0], "nom"),
            &Node::Array(neutral.clone())
        );
        assert_eq!(
            field(&anonymized.documents[0], "email"),
            &Node::Array(neutral)
        );
        assert!(anonymized.pseudonymized.is_empty());
        assert_no_canary(&anonymized);
    }

    #[test]
    fn les_binaires_expressions_et_javascript_deviennent_opaques_avec_leur_longueur() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![
            (
                "piece",
                Node::Binary {
                    subtype: 0,
                    len: 12,
                },
            ),
            (
                "motif",
                Node::Opaque {
                    type_name: "regex".to_string(),
                    len: 9,
                },
            ),
            (
                "script",
                Node::Opaque {
                    type_name: "javascript".to_string(),
                    len: 20,
                },
            ),
            (
                "pointeur",
                Node::Opaque {
                    type_name: "dbpointer".to_string(),
                    len: 36,
                },
            ),
        ]);

        let anonymized = anonymizer.documents(vec![document], &[]);

        let document = &anonymized.documents[0];
        assert_eq!(field(document, "piece"), &text("<binary len=12>"));
        assert_eq!(field(document, "motif"), &text("<regex len=9>"));
        assert_eq!(field(document, "script"), &text("<javascript len=20>"));
        assert_eq!(field(document, "pointeur"), &text("<dbpointer len=36>"));
        assert_eq!(
            flagged(&anonymized, "piece"),
            flag("person_name", Reason::Default)
        );
        assert_no_canary(&anonymized);
    }

    #[test]
    fn les_nombres_suivent_la_regle_numerique_quel_que_soit_leur_type_bson() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![
            ("telephoneFixe", Node::Double(33641278395.0)),
            ("siren", Node::Decimal("418273652".to_string())),
            ("prix", Node::Double(12.5)),
            ("montant", Node::Decimal("33612345678.00".to_string())),
            ("telephoneSecours", Node::Double(12.5)),
        ]);

        let anonymized = anonymizer.documents(vec![document], &[]);

        let document = &anonymized.documents[0];
        assert_ne!(
            field(document, "telephoneFixe"),
            &Node::Double(33641278395.0)
        );
        assert_eq!(
            flagged(&anonymized, "telephoneFixe"),
            flag("phone", Reason::Dictionary)
        );
        assert_ne!(
            field(document, "siren"),
            &Node::Decimal("418273652".to_string())
        );
        assert_eq!(
            flagged(&anonymized, "siren"),
            flag("siren", Reason::Dictionary)
        );
        assert_eq!(field(document, "prix"), &Node::Double(12.5));
        assert_eq!(
            field(document, "montant"),
            &Node::Decimal("33612345678.00".to_string())
        );
        assert_eq!(field(document, "telephoneSecours"), &Node::Double(12.5));
        assert_no_canary(&anonymized);
    }

    #[test]
    fn un_tableau_herite_du_chemin_de_son_champ_sans_indice() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![
            (
                "email",
                Node::Array(vec![
                    text("ylwenna.kervadec@canary-mail.fr"),
                    Node::Array(vec![text("o.hazvekmor@canary-mail.fr")]),
                ]),
            ),
            (
                "items",
                Node::Array(vec![
                    object(vec![("nom", text("Kervadec")), ("sku", text("A-12"))]),
                    object(vec![("nom", text("Hazvekmor")), ("sku", text("B-7"))]),
                ]),
            ),
        ]);

        let anonymized = anonymizer.documents(vec![document], &[]);

        assert_eq!(
            anonymized.pseudonymized.keys().collect::<Vec<_>>(),
            ["email", "items.nom"]
        );
        let Node::Array(items) = field(&anonymized.documents[0], "items") else {
            panic!("items stays an array");
        };
        assert_eq!(field(&items[1], "sku"), &text("B-7"));
        assert_ne!(field(&items[0], "nom"), &text("Kervadec"));
        assert_ne!(field(&items[1], "nom"), &text("Hazvekmor"));
        assert_eq!(
            flagged(&anonymized, "items.nom"),
            flag("last_name", Reason::Dictionary)
        );
        assert_no_canary(&anonymized);
    }

    #[test]
    fn un_numero_sous_adresse_herite_du_type_adresse_mais_pas_un_identifiant() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![(
            "adresse",
            object(vec![("numero", Node::Int(12)), ("id", text("ABC-12"))]),
        )]);

        let anonymized = anonymizer.documents(vec![document], &[]);

        let document = &anonymized.documents[0];
        let number = field(document, "adresse.numero");
        assert!(matches!(number, Node::Text(_)), "{number:?}");
        assert_eq!(
            flagged(&anonymized, "adresse.numero"),
            flag("address", Reason::Dictionary)
        );
        assert_eq!(field(document, "adresse.id"), &text("ABC-12"));
        assert_eq!(flagged(&anonymized, "adresse.id"), None);
        assert_no_canary(&anonymized);
    }

    #[test]
    fn un_entier_sous_telephone_est_pseudonymise_meme_sous_une_cle_inconnue() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![(
            "telephone",
            object(vec![
                ("mobile", Node::Int(612345678)),
                ("valeur", Node::Int(33641278395)),
                ("prenom", text("Ylwenna")),
            ]),
        )]);

        let anonymized = anonymizer.documents(vec![document], &[]);

        let document = &anonymized.documents[0];
        assert_ne!(field(document, "telephone.mobile"), &Node::Int(612345678));
        assert_eq!(
            flagged(&anonymized, "telephone.mobile"),
            flag("phone", Reason::Dictionary)
        );
        assert_ne!(field(document, "telephone.valeur"), &Node::Int(33641278395));
        assert_eq!(
            flagged(&anonymized, "telephone.valeur"),
            flag("phone", Reason::Dictionary)
        );
        assert_eq!(
            flagged(&anonymized, "telephone.prenom"),
            flag("first_name", Reason::Dictionary)
        );
        assert_no_canary(&anonymized);
    }

    #[test]
    fn le_type_traverse_un_tableau_et_les_detecteurs_veillent_sous_des_cles_inconnues() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![
            (
                "telephone",
                Node::Array(vec![Node::Int(33641278395), Node::Int(33758149236)]),
            ),
            (
                "contact",
                object(vec![(
                    "perso",
                    object(vec![("courrier", text("ylwenna.kervadec@canary-mail.fr"))]),
                )]),
            ),
        ]);

        let anonymized = anonymizer.documents(vec![document], &[]);

        let document = &anonymized.documents[0];
        let Node::Array(phones) = field(document, "telephone") else {
            panic!("telephone stays an array");
        };
        assert!(!phones.contains(&Node::Int(33641278395)), "{phones:?}");
        assert!(!phones.contains(&Node::Int(33758149236)), "{phones:?}");
        assert_eq!(
            flagged(&anonymized, "telephone"),
            flag("phone", Reason::Dictionary)
        );
        assert_eq!(
            field(document, "contact.perso.courrier"),
            &text(
                &pseudonymizer
                    .pseudonymize(Kind::Email, "ylwenna.kervadec@canary-mail.fr")
                    .text
            )
        );
        assert_eq!(
            flagged(&anonymized, "contact.perso.courrier"),
            flag("email", Reason::Detected)
        );
        assert_no_canary(&anonymized);
    }

    #[test]
    fn une_valeur_reelle_traduite_depuis_un_filtre_est_remplacee_par_son_pseudonyme() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let pseudonym = pseudonymizer.pseudonymize(Kind::LastName, "Kervadec").text;
        let injected = [("Kervadec".to_string(), pseudonym.clone())];
        let anonymizer = Anonymizer::new(&pseudonymizer, &injected);
        let document = || object(vec![("note", text("rappeler Kervadec demain"))]);

        let anonymized = anonymizer.documents(vec![document()], &[]);
        let without_pair = Anonymizer::new(&pseudonymizer, &[]).documents(vec![document()], &[]);

        assert_eq!(
            field(&anonymized.documents[0], "note"),
            &text(&format!("rappeler {pseudonym} demain"))
        );
        assert_ne!(
            field(&without_pair.documents[0], "note"),
            field(&anonymized.documents[0], "note")
        );
        assert_no_canary(&anonymized);
    }

    #[test]
    fn une_valeur_injectee_est_remplacee_dans_une_cle_un_tableau_et_sous_un_chemin_en_clair() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let pseudonym = pseudonymizer.pseudonymize(Kind::LastName, "Kervadec").text;
        let injected = [("Kervadec".to_string(), pseudonym.clone())];
        let anonymizer = Anonymizer::new(&pseudonymizer, &injected);
        let document = object(vec![
            ("Kervadec", Node::Int(1)),
            ("notes", Node::Array(vec![text("voir Kervadec")])),
            ("libelle", text("rappeler Kervadec")),
        ]);
        let clearances = [clearance("libelle", FieldClearance::Clear)];

        let anonymized = anonymizer.documents(vec![document], &clearances);

        let document = &anonymized.documents[0];
        assert_eq!(field(document, &pseudonym), &Node::Int(1));
        assert_eq!(
            field(document, "notes"),
            &Node::Array(vec![text(&format!("voir {pseudonym}"))])
        );
        assert_eq!(
            field(document, "libelle"),
            &text(&format!("rappeler {pseudonym}"))
        );
        assert_no_canary(&anonymized);
    }

    #[test]
    fn des_telephones_au_pluriel_en_int64_prennent_le_type_du_singulier() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![(
            "contact",
            object(vec![(
                "phones",
                Node::Array(vec![Node::Int(33641278395), Node::Int(33758149236)]),
            )]),
        )]);

        let anonymized = anonymizer.documents(vec![document], &[]);

        let Node::Array(phones) = field(&anonymized.documents[0], "contact.phones") else {
            panic!("phones stays an array");
        };
        assert!(!phones.contains(&Node::Int(33641278395)), "{phones:?}");
        assert!(!phones.contains(&Node::Int(33758149236)), "{phones:?}");
        assert_eq!(
            flagged(&anonymized, "contact.phones"),
            flag("phone", Reason::Dictionary)
        );
        assert_no_canary(&anonymized);
    }

    #[test]
    fn des_prenoms_au_pluriel_en_minuscules_prennent_le_type_du_singulier() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![("prenoms", Node::Array(vec![text("ylwenna")]))]);

        let anonymized = anonymizer.documents(vec![document], &[]);

        assert_ne!(
            field(&anonymized.documents[0], "prenoms"),
            &Node::Array(vec![text("ylwenna")])
        );
        assert_eq!(
            flagged(&anonymized, "prenoms"),
            flag("first_name", Reason::Dictionary)
        );
        assert_no_canary(&anonymized);
    }

    #[test]
    fn des_codes_postaux_au_pluriel_en_aux_prennent_le_type_du_singulier_en_al() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![("codesPostaux", Node::Array(vec![Node::Int(75011)]))]);

        let anonymized = anonymizer.documents(vec![document], &[]);

        assert_ne!(
            field(&anonymized.documents[0], "codesPostaux"),
            &Node::Array(vec![Node::Int(75011)])
        );
        assert_eq!(
            flagged(&anonymized, "codesPostaux"),
            flag("postal_code", Reason::Dictionary)
        );
        assert_no_canary(&anonymized);
    }

    #[test]
    fn des_dossiers_medicaux_au_pluriel_sont_traites_comme_le_dossier_medical_au_singulier() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![(
            "telephone",
            object(vec![
                ("dossierMedical", Node::Array(vec![text("DM-42")])),
                ("dossiersMedicaux", Node::Array(vec![text("DM-43")])),
            ]),
        )]);

        let anonymized = anonymizer.documents(vec![document], &[]);

        let document = &anonymized.documents[0];
        assert_eq!(
            field(document, "telephone.dossierMedical"),
            &Node::Array(vec![text("DM-42")])
        );
        assert_eq!(
            field(document, "telephone.dossiersMedicaux"),
            &Node::Array(vec![text("DM-43")])
        );
        assert_eq!(flagged(&anonymized, "telephone.dossiersMedicaux"), None);
        assert_no_canary(&anonymized);
    }

    #[test]
    fn une_clearance_en_clair_sur_un_objet_ne_couvre_pas_ses_enfants_et_ses_cles_restent_scannees()
    {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![(
            "contact",
            object(vec![
                ("nom", text("Kervadec")),
                ("ylwenna.kervadec@canary-mail.fr", Node::Bool(true)),
            ]),
        )]);
        let clearances = [clearance("contact", FieldClearance::Clear)];

        let anonymized = anonymizer.documents(vec![document], &clearances);

        let document = &anonymized.documents[0];
        assert_ne!(field(document, "contact.nom"), &text("Kervadec"));
        assert_eq!(
            flagged(&anonymized, "contact.nom"),
            flag("last_name", Reason::Dictionary)
        );
        let email = pseudonymizer
            .pseudonymize(Kind::Email, "ylwenna.kervadec@canary-mail.fr")
            .text;
        let Node::Object(entries) = field(document, "contact") else {
            panic!("contact stays an object");
        };
        assert_eq!(entries[1], (email.clone(), Node::Bool(true)));
        assert_eq!(
            flagged(&anonymized, &format!("contact.{email}")),
            flag("email", Reason::Detected)
        );
        assert_no_canary(&anonymized);
    }

    #[test]
    fn une_clearance_personnelle_couvre_tout_le_sous_arbre_sauf_un_enfant_declare_en_clair() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![(
            "dossier",
            object(vec![
                (
                    "piece",
                    object(vec![
                        ("reference", text("lot-42")),
                        ("tags", Node::Array(vec![text("lot-43"), text("lot-44")])),
                    ]),
                ),
                ("libelle", text("Garance Bleue")),
            ]),
        )]);
        let clearances = [
            clearance("dossier", FieldClearance::Personal(Kind::LastName)),
            clearance("dossier.libelle", FieldClearance::Clear),
        ];

        let anonymized = anonymizer.documents(vec![document], &clearances);

        let document = &anonymized.documents[0];
        assert_ne!(field(document, "dossier.piece.reference"), &text("lot-42"));
        assert_eq!(
            flagged(&anonymized, "dossier.piece.reference"),
            flag("last_name", Reason::Policy)
        );
        let Node::Array(tags) = field(document, "dossier.piece.tags") else {
            panic!("tags stays an array");
        };
        assert!(!tags.contains(&text("lot-43")), "{tags:?}");
        assert!(!tags.contains(&text("lot-44")), "{tags:?}");
        assert_eq!(
            flagged(&anonymized, "dossier.piece.tags"),
            flag("last_name", Reason::Policy)
        );
        assert_eq!(field(document, "dossier.libelle"), &text("Garance Bleue"));
        assert_eq!(flagged(&anonymized, "dossier.libelle"), None);
        assert_no_canary(&anonymized);
    }

    #[test]
    fn une_clearance_sur_un_chemin_pointe_ou_a_travers_un_tableau_vise_ce_seul_champ() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![
            ("contact", object(vec![("nom", text("Garance Bleue"))])),
            (
                "items",
                Node::Array(vec![
                    object(vec![("nom", text("Garance"))]),
                    object(vec![("nom", text("Bleue"))]),
                ]),
            ),
            ("client", object(vec![("nom", text("Kervadec"))])),
        ]);
        let clearances = [
            clearance("contact.nom", FieldClearance::Clear),
            clearance("items.nom", FieldClearance::Clear),
        ];

        let anonymized = anonymizer.documents(vec![document], &clearances);

        let document = &anonymized.documents[0];
        assert_eq!(field(document, "contact.nom"), &text("Garance Bleue"));
        let Node::Array(items) = field(document, "items") else {
            panic!("items stays an array");
        };
        assert_eq!(field(&items[0], "nom"), &text("Garance"));
        assert_eq!(field(&items[1], "nom"), &text("Bleue"));
        assert_ne!(field(document, "client.nom"), &text("Kervadec"));
        assert_eq!(
            anonymized.pseudonymized.keys().collect::<Vec<_>>(),
            ["client.nom"]
        );
        assert_no_canary(&anonymized);
    }

    #[test]
    fn un_champ_derive_souille_devient_opaque_ou_pseudonymise_avec_la_raison_derivee() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let derived = FieldClearance::Derived {
            proven_clear: false,
        };
        let document = object(vec![
            ("who", text("ylwenna.kervadec")),
            ("n", Node::Int(31)),
            ("jour", Node::Date("1987-03-14T00:00:00.000Z".to_string())),
            ("groupe", object(vec![("cle", text("ylwenna.kervadec"))])),
        ]);
        let clearances = [
            clearance("who", derived.clone()),
            clearance("n", derived.clone()),
            clearance("jour", derived.clone()),
            clearance("groupe", derived),
        ];

        let anonymized = anonymizer.documents(vec![document], &clearances);

        let document = &anonymized.documents[0];
        let opaque = text(&pseudonymizer.opaque_token("derived", "ylwenna.kervadec"));
        assert_eq!(field(document, "who"), &opaque);
        assert_eq!(
            flagged(&anonymized, "who"),
            flag("derived_personal", Reason::Derived)
        );
        assert_eq!(field(document, "n"), &Node::Int(31));
        assert_ne!(
            field(document, "jour"),
            &Node::Date("1987-03-14T00:00:00.000Z".to_string())
        );
        assert_eq!(
            flagged(&anonymized, "jour"),
            flag("birth_date", Reason::Derived)
        );
        assert_eq!(
            field(document, "groupe"),
            &object(vec![(
                &pseudonymizer.opaque_token("derived", "cle"),
                opaque.clone()
            )])
        );
        assert_no_canary(&anonymized);
    }

    #[test]
    fn un_champ_derive_prouve_en_clair_garde_ses_valeurs_mais_garde_le_filet_structurel() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let proven = FieldClearance::Derived { proven_clear: true };
        let document = object(vec![
            ("etiquette", text("Garance Bleue paid")),
            ("contact", text("Garance ylwenna.kervadec@canary-mail.fr")),
            ("jour", Node::Date("2026-03-02T09:15:00.000Z".to_string())),
        ]);
        let clearances = [
            clearance("etiquette", proven.clone()),
            clearance("contact", proven.clone()),
            clearance("jour", proven),
        ];

        let anonymized = anonymizer.documents(vec![document], &clearances);

        let document = &anonymized.documents[0];
        assert_eq!(field(document, "etiquette"), &text("Garance Bleue paid"));
        let Node::Text(contact) = field(document, "contact") else {
            panic!("contact stays text");
        };
        assert!(contact.starts_with("Garance "), "{contact}");
        assert!(!contact.contains("canary-mail"), "{contact}");
        assert_eq!(
            field(document, "jour"),
            &Node::Date("2026-03-02T09:15:00.000Z".to_string())
        );
        assert_eq!(flagged(&anonymized, "etiquette"), None);
        assert_no_canary(&anonymized);
    }

    #[test]
    fn un_parent_derive_souille_n_est_jamais_eclairci_par_un_enfant_en_clair() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![(
            "whole",
            object(vec![
                ("nom", text("Kervadec")),
                (
                    "statut",
                    object(vec![("libelle", text("Ylwenna Kervadec"))]),
                ),
            ]),
        )]);
        let clearances = [
            clearance(
                "whole",
                FieldClearance::Derived {
                    proven_clear: false,
                },
            ),
            clearance("whole.nom", FieldClearance::Clear),
            clearance(
                "whole.statut.libelle",
                FieldClearance::Derived { proven_clear: true },
            ),
        ];

        let anonymized = anonymizer.documents(vec![document], &clearances);

        let text_of_tree = format!("{:?}", anonymized.documents);
        assert!(!text_of_tree.contains("Kervadec"), "{text_of_tree}");
        assert!(!text_of_tree.contains("Ylwenna"), "{text_of_tree}");
        assert!(
            anonymized
                .pseudonymized
                .values()
                .all(|flag| flag.why == Reason::Derived),
            "{:?}",
            anonymized.pseudonymized
        );
        assert_no_canary(&anonymized);
    }

    #[test]
    fn un_champ_renomme_garde_le_type_du_nom_de_sa_source() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![
            ("n", Node::Int(33_641_278_395)),
            ("quantite", Node::Int(2)),
        ]);
        let clearances = [
            clearance("n", FieldClearance::NamedAs("telephone".into())),
            clearance("quantite", FieldClearance::NamedAs("quantity".into())),
        ];

        let anonymized = anonymizer.documents(vec![document], &clearances);

        let document = &anonymized.documents[0];
        assert_ne!(field(document, "n"), &Node::Int(33_641_278_395));
        assert_eq!(flagged(&anonymized, "n"), flag("phone", Reason::Dictionary));
        assert_eq!(field(document, "quantite"), &Node::Int(2));
        assert_no_canary(&anonymized);
    }

    #[test]
    fn une_cle_qui_contient_un_point_ne_prend_pas_la_clearance_du_chemin_imbrique() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let document = object(vec![
            ("client.nom", text("Kervadec")),
            ("client", object(vec![("nom", text("Garance Bleue"))])),
        ]);
        let clearances = [clearance("client.nom", FieldClearance::Clear)];

        let anonymized = anonymizer.documents(vec![document], &clearances);

        let document = &anonymized.documents[0];
        let Node::Object(entries) = document else {
            panic!("an object");
        };
        assert_ne!(entries[0].1, text("Kervadec"));
        assert_eq!(field(document, "client.nom"), &text("Garance Bleue"));
        assert_no_canary(&anonymized);
    }

    #[test]
    fn un_message_du_serveur_masque_les_valeurs_citees_entre_apostrophes() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let message = anonymizer.server_message(
            "Failed to parse number 'Kervadec' in $convert with no onError value: '$toInt', got \"Ylwenna\"",
        );
        assert!(!message.contains("Kervadec"), "{message}");
        assert!(!message.contains("Ylwenna"), "{message}");
        assert!(message.contains("'$toInt'"), "{message}");
        assert!(message.starts_with("Failed to parse number '"), "{message}");
    }

    #[test]
    fn une_cle_echantillonnee_qui_est_un_email_est_pseudonymisee() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let key = anonymizer.document_key("kervadec.ylwenna@canary-mail.fr");
        assert!(!key.contains("canary-mail"), "{key}");
        assert_eq!(anonymizer.document_key("newsletter"), "newsletter");
    }
}
