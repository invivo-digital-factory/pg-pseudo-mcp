use mongodb::bson::Bson;

use crate::composition::adapters::stored_node;
use crate::contexts::connections::infrastructure::data_sources::mongodb::decode;
use crate::contexts::querying::domain::services::ejson_render::render;
use crate::contexts::querying::domain::types::DocumentNode;

#[test]
fn le_rendu_extended_json_relache_se_relit_par_le_pilote_a_l_identique() {
    let nodes = vec![
        DocumentNode::Null,
        DocumentNode::Bool(false),
        DocumentNode::Int(7),
        DocumentNode::Int(33_641_278_395),
        DocumentNode::Double(0.25),
        DocumentNode::Double(f64::INFINITY),
        DocumentNode::Decimal("12345.6700".into()),
        DocumentNode::Text("paid".into()),
        DocumentNode::Date("2026-03-01T09:30:00.000Z".into()),
        DocumentNode::Date("1950-06-15T12:00:00.123Z".into()),
        DocumentNode::ObjectId("65a000000000000000000001".into()),
        DocumentNode::Uuid {
            subtype: 4,
            value: "0b1f2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d".into(),
        },
        DocumentNode::Uuid {
            subtype: 3,
            value: "0b1f2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d".into(),
        },
        DocumentNode::Timestamp("1767225600:1".into()),
        DocumentNode::MinKey,
        DocumentNode::MaxKey,
        DocumentNode::Array(vec![DocumentNode::Int(1), DocumentNode::Text("x".into())]),
        DocumentNode::Object(vec![
            ("z".into(), DocumentNode::Int(1)),
            ("a".into(), DocumentNode::Text("b".into())),
        ]),
    ];
    for node in nodes {
        let rendered = render(node.clone());
        let bson = Bson::try_from(rendered.clone())
            .unwrap_or_else(|error| panic!("{rendered} is not Extended JSON: {error}"));
        assert_eq!(stored_node(decode(bson)), node, "{rendered}");
    }
}
