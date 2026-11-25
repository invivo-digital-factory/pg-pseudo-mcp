use crate::contexts::anonymization::domain::types as anonymization;
use crate::contexts::connections::domain::types as connections;
use crate::contexts::querying::domain::types as querying;

const LEGACY_UUID_SUBTYPE: u8 = 3;
const STANDARD_UUID_SUBTYPE: u8 = 4;

pub fn stored_node(node: connections::DocumentNode) -> querying::DocumentNode {
    use connections::DocumentNode as In;
    use querying::DocumentNode as Out;
    match node {
        In::Null => Out::Null,
        In::Bool(flag) => Out::Bool(flag),
        In::Int(number) => Out::Int(number),
        In::Double(number) => Out::Double(number),
        In::Decimal(text) => Out::Decimal(text),
        In::Text(text) => Out::Text(text),
        In::Date(text) => Out::Date(text),
        In::ObjectId(hex) => Out::ObjectId(hex),
        In::Uuid { subtype, value } => Out::Uuid {
            subtype: match subtype {
                connections::UuidSubtype::Legacy => LEGACY_UUID_SUBTYPE,
                connections::UuidSubtype::Standard => STANDARD_UUID_SUBTYPE,
            },
            value,
        },
        In::Timestamp(text) => Out::Timestamp(text),
        In::MinKey => Out::MinKey,
        In::MaxKey => Out::MaxKey,
        In::Binary { subtype, len } => Out::Binary { subtype, len },
        In::Opaque { type_name, len } => Out::Opaque { type_name, len },
        In::Array(items) => Out::Array(items.into_iter().map(stored_node).collect()),
        In::Object(entries) => Out::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key, stored_node(value)))
                .collect(),
        ),
    }
}

pub fn node_in(node: querying::DocumentNode) -> anonymization::Node {
    use anonymization::Node as Out;
    use querying::DocumentNode as In;
    match node {
        In::Null => Out::Null,
        In::Bool(flag) => Out::Bool(flag),
        In::Int(number) => Out::Int(number),
        In::Double(number) => Out::Double(number),
        In::Decimal(text) => Out::Decimal(text),
        In::Text(text) => Out::Text(text),
        In::Date(text) => Out::Date(text),
        In::ObjectId(hex) => Out::ObjectId(hex),
        In::Uuid { subtype, value } => Out::Uuid { subtype, value },
        In::Timestamp(text) => Out::Timestamp(text),
        In::MinKey => Out::MinKey,
        In::MaxKey => Out::MaxKey,
        In::Binary { subtype, len } => Out::Binary { subtype, len },
        In::Opaque { type_name, len } => Out::Opaque { type_name, len },
        In::Array(items) => Out::Array(items.into_iter().map(node_in).collect()),
        In::Object(entries) => Out::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key, node_in(value)))
                .collect(),
        ),
    }
}

pub fn node_out(node: anonymization::Node) -> querying::DocumentNode {
    use anonymization::Node as In;
    use querying::DocumentNode as Out;
    match node {
        In::Null => Out::Null,
        In::Bool(flag) => Out::Bool(flag),
        In::Int(number) => Out::Int(number),
        In::Double(number) => Out::Double(number),
        In::Decimal(text) => Out::Decimal(text),
        In::Text(text) => Out::Text(text),
        In::Date(text) => Out::Date(text),
        In::ObjectId(hex) => Out::ObjectId(hex),
        In::Uuid { subtype, value } => Out::Uuid { subtype, value },
        In::Timestamp(text) => Out::Timestamp(text),
        In::MinKey => Out::MinKey,
        In::MaxKey => Out::MaxKey,
        In::Binary { subtype, len } => Out::Binary { subtype, len },
        In::Opaque { type_name, len } => Out::Opaque { type_name, len },
        In::Array(items) => Out::Array(items.into_iter().map(node_out).collect()),
        In::Object(entries) => Out::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key, node_out(value)))
                .collect(),
        ),
    }
}

pub fn field_clearance(clearance: &querying::DocumentClearance) -> anonymization::FieldClearance {
    match clearance {
        querying::DocumentClearance::Clear => anonymization::FieldClearance::Clear,
        querying::DocumentClearance::Personal(kind) => anonymization::Kind::parse(kind).map_or(
            anonymization::FieldClearance::Derived {
                proven_clear: false,
            },
            anonymization::FieldClearance::Personal,
        ),
        querying::DocumentClearance::Derived { proven_clear } => {
            anonymization::FieldClearance::Derived {
                proven_clear: *proven_clear,
            }
        }
        querying::DocumentClearance::NamedAs(name) => {
            anonymization::FieldClearance::NamedAs(name.clone())
        }
    }
}

pub fn collection_kind(kind: connections::CollectionKind) -> &'static str {
    match kind {
        connections::CollectionKind::Collection => "collection",
        connections::CollectionKind::View => "view",
        connections::CollectionKind::Timeseries => "timeseries",
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn le_sous_type_d_un_uuid_traverse_les_trois_contextes() {
        for (subtype, expected) in [
            (connections::UuidSubtype::Legacy, 3),
            (connections::UuidSubtype::Standard, 4),
        ] {
            let stored = stored_node(connections::DocumentNode::Uuid {
                subtype,
                value: "0b1f2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d".into(),
            });
            let back = node_out(node_in(stored.clone()));
            assert_eq!(back, stored);
            assert_eq!(
                back,
                querying::DocumentNode::Uuid {
                    subtype: expected,
                    value: "0b1f2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d".into()
                }
            );
        }
    }

    #[test]
    fn un_type_personnel_inconnu_reste_souille_plutot_qu_en_clair() {
        assert_eq!(
            field_clearance(&querying::DocumentClearance::Personal("nope".into())),
            anonymization::FieldClearance::Derived {
                proven_clear: false
            }
        );
    }
}
