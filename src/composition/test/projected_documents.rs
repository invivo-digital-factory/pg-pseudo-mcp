use serde_json::{Value, json};
use tempfile::TempDir;

use crate::composition::adapters::DocumentAnonymizerAdapter;
use crate::contexts::anonymization::injector::AnonymizationInjector;
use crate::contexts::querying::domain::repositories::DocumentAnonymizer;
use crate::contexts::querying::domain::services::document_clearances::{
    DocumentCommand, document_clearances, observed_paths,
};
use crate::contexts::querying::domain::services::mongo_guard::{inspect_aggregate, inspect_find};
use crate::contexts::querying::domain::services::mongo_lineage::document_lineage;
use crate::contexts::querying::domain::types::{
    AggregateRequest, AnonymizedDocuments, ColumnPolicy, DocumentNode, FindRequest, GuardedMongo,
    MongoFieldPattern, PolicyDecision, QueryLimits, Reason, Translations,
};

const LEVEL_LABEL: &str = "attributes.partnership_level.label";
const SUPPLIER_LABEL: &str = "attributes.supplier.attributes.label";
const SUPPLIER_CONTACT: &str = "attributes.supplier.attributes.contact";

fn policy() -> ColumnPolicy {
    let mut policy = ColumnPolicy::default();
    for path in [LEVEL_LABEL, SUPPLIER_LABEL] {
        policy.insert_field(
            MongoFieldPattern {
                database: Some("shop".into()),
                collection: Some("products".into()),
                path: path.into(),
            },
            PolicyDecision::Clear,
        );
    }
    policy
}

fn object(entries: Vec<(&str, DocumentNode)>) -> DocumentNode {
    DocumentNode::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

fn text(value: &str) -> DocumentNode {
    DocumentNode::Text(value.into())
}

fn product() -> DocumentNode {
    object(vec![
        (
            "_id",
            DocumentNode::ObjectId("650000000000000000000001".into()),
        ),
        (
            "attributes",
            object(vec![
                (
                    "partnership_level",
                    object(vec![("label", text("Gold partner"))]),
                ),
                (
                    "supplier",
                    object(vec![(
                        "attributes",
                        object(vec![
                            ("label", text("Acme Seeds orders@acme-seeds.example")),
                            ("contact", text("Jean Dupont")),
                        ]),
                    )]),
                ),
            ]),
        ),
    ])
}

fn inclusion() -> Value {
    json!({LEVEL_LABEL: 1, SUPPLIER_LABEL: 1, SUPPLIER_CONTACT: 1})
}

fn found(projection: Option<Value>) -> GuardedMongo {
    inspect_find(&FindRequest {
        database: Some("shop".into()),
        collection: "products".into(),
        projection,
        ..FindRequest::default()
    })
    .expect("allowed")
    .finish(&Translations::default(), &QueryLimits::default())
}

fn aggregated(pipeline: Vec<Value>) -> GuardedMongo {
    inspect_aggregate(&AggregateRequest {
        database: Some("shop".into()),
        collection: "products".into(),
        pipeline,
        ..AggregateRequest::default()
    })
    .expect("allowed")
    .finish(&Translations::default(), &QueryLimits::default())
}

fn anonymized(
    anonymizer: &DocumentAnonymizerAdapter,
    (guarded, command): (&GuardedMongo, DocumentCommand),
    policy: &ColumnPolicy,
    document: DocumentNode,
) -> AnonymizedDocuments {
    let documents = vec![document];
    let clearances = document_clearances(
        &observed_paths(&documents),
        &document_lineage(guarded, "shop"),
        policy,
        ("shop", "products"),
        command,
    );
    anonymizer.documents(documents, &clearances, &[])
}

fn anonymizer(dir: &TempDir) -> DocumentAnonymizerAdapter {
    DocumentAnonymizerAdapter::new(
        AnonymizationInjector::new(&dir.path().join("data"))
            .use_cases()
            .unwrap(),
    )
}

fn field<'a>(node: &'a DocumentNode, path: &str) -> &'a DocumentNode {
    path.split('.').fold(node, |node, key| match node {
        DocumentNode::Object(entries) => entries
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
            .unwrap_or_else(|| panic!("no key {key} in {node:?}")),
        other => panic!("{key} below a scalar {other:?}"),
    })
}

mod unit {
    use super::*;

    #[test]
    fn une_projection_d_inclusion_sur_des_chemins_imbriques_pseudonymise_comme_sans_projection() {
        let dir = TempDir::new().unwrap();
        let anonymizer = anonymizer(&dir);
        for (command, projected, whole) in [
            (DocumentCommand::Find, found(Some(inclusion())), found(None)),
            (
                DocumentCommand::Aggregate,
                aggregated(vec![json!({"$project": inclusion()})]),
                aggregated(Vec::new()),
            ),
        ] {
            let projected = anonymized(&anonymizer, (&projected, command), &policy(), product());
            let whole = anonymized(&anonymizer, (&whole, command), &policy(), product());
            assert_eq!(projected, whole, "{command:?}");

            let document = &projected.documents[0];
            assert_eq!(
                field(document, LEVEL_LABEL),
                &text("Gold partner"),
                "{command:?}"
            );
            let supplier_label = format!("{:?}", field(document, SUPPLIER_LABEL));
            assert!(
                supplier_label.contains("Acme Seeds")
                    && !supplier_label.contains("orders@acme-seeds.example"),
                "{command:?}: {supplier_label}"
            );
            let rendered = format!("{document:?}");
            assert!(!rendered.contains("Dupont"), "{command:?}: {rendered}");
            assert!(
                projected
                    .pseudonymized
                    .values()
                    .all(|flag| flag.why != Reason::Derived),
                "{command:?}: {:?}",
                projected.pseudonymized
            );
        }
    }

    #[test]
    fn des_valeurs_posees_par_set_dans_un_document_declare_en_clair_restent_pseudonymisees() {
        let dir = TempDir::new().unwrap();
        let mut policy = ColumnPolicy::default();
        for path in ["attributes", "attributes.color"] {
            policy.insert_field(
                MongoFieldPattern {
                    database: Some("shop".into()),
                    collection: Some("products".into()),
                    path: path.into(),
                },
                PolicyDecision::Clear,
            );
        }
        let guarded = aggregated(vec![json!({"$set": {
            "attributes.buyer": "$lastName",
            "attributes.reach": "$email"
        }})]);
        let document = object(vec![(
            "attributes",
            object(vec![
                ("color", text("Red")),
                ("buyer", text("Dupont")),
                ("reach", text("jean.dupont@canary-mail.fr")),
            ]),
        )]);
        let anonymized = anonymized(
            &anonymizer(&dir),
            (&guarded, DocumentCommand::Aggregate),
            &policy,
            document,
        );
        let document = &anonymized.documents[0];
        assert_eq!(field(document, "attributes.color"), &text("Red"));
        let rendered = format!("{document:?}").to_lowercase();
        assert!(!rendered.contains("dupont"), "{rendered}");
        for path in ["attributes.buyer", "attributes.reach"] {
            assert!(
                anonymized.pseudonymized.contains_key(path),
                "{path}: {:?}",
                anonymized.pseudonymized
            );
        }
    }
}
