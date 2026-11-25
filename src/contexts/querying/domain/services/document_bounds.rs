use serde_json::{Map, Value};

use crate::contexts::querying::domain::types::DocumentNode;

const SCALAR_BYTES: usize = 24;
const KEY_OVERHEAD: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodePosition {
    pub document: usize,
    pub steps: Vec<usize>,
}

pub fn cap_arrays(documents: &mut [DocumentNode], max_items: usize) -> Vec<NodePosition> {
    let mut cut = Vec::new();
    for (document, node) in documents.iter_mut().enumerate() {
        cap_node(node, max_items, &mut Vec::new(), document, &mut cut);
    }
    cut
}

fn cap_node(
    node: &mut DocumentNode,
    max_items: usize,
    steps: &mut Vec<usize>,
    document: usize,
    cut: &mut Vec<NodePosition>,
) {
    match node {
        DocumentNode::Array(items) => {
            if items.len() > max_items {
                items.truncate(max_items);
                cut.push(NodePosition {
                    document,
                    steps: steps.clone(),
                });
            }
            for (index, item) in items.iter_mut().enumerate() {
                steps.push(index);
                cap_node(item, max_items, steps, document, cut);
                steps.pop();
            }
        }
        DocumentNode::Object(entries) => {
            for (index, (_, value)) in entries.iter_mut().enumerate() {
                steps.push(index);
                cap_node(value, max_items, steps, document, cut);
                steps.pop();
            }
        }
        _ => {}
    }
}

pub fn shown_path(documents: &[DocumentNode], position: &NodePosition) -> String {
    let mut segments = Vec::new();
    let mut current = documents.get(position.document);
    for step in &position.steps {
        current = match current {
            Some(DocumentNode::Object(entries)) => entries.get(*step).map(|(key, value)| {
                segments.push(key.clone());
                value
            }),
            Some(DocumentNode::Array(items)) => items.get(*step),
            _ => None,
        };
    }
    segments.join(".")
}

pub fn decoded_size(node: &DocumentNode) -> usize {
    match node {
        DocumentNode::Text(text) | DocumentNode::Decimal(text) => text.len() + 2,
        DocumentNode::Array(items) => items.iter().map(decoded_size).sum::<usize>() + 2,
        DocumentNode::Object(entries) => {
            entries
                .iter()
                .map(|(key, value)| key.len() + KEY_OVERHEAD + decoded_size(value))
                .sum::<usize>()
                + 2
        }
        _ => SCALAR_BYTES,
    }
}

pub fn prune_decoded(documents: Vec<DocumentNode>, budget: usize) -> (Vec<DocumentNode>, bool) {
    let mut kept = Vec::with_capacity(documents.len());
    let mut used = 0usize;
    for document in documents {
        let size = decoded_size(&document);
        if used + size <= budget {
            used += size;
            kept.push(document);
            continue;
        }
        if kept.is_empty() {
            kept.push(shrink(document, budget));
        }
        return (kept, true);
    }
    (kept, false)
}

fn shrink(node: DocumentNode, budget: usize) -> DocumentNode {
    match node {
        DocumentNode::Object(entries) => {
            let mut kept = Vec::new();
            let mut used = 2usize;
            for (key, value) in entries {
                let size = key.len() + KEY_OVERHEAD + decoded_size(&value);
                if used + size <= budget {
                    used += size;
                    kept.push((key, value));
                    continue;
                }
                let room = budget.saturating_sub(used + key.len() + KEY_OVERHEAD);
                if room >= SCALAR_BYTES {
                    kept.push((key, shrink(value, room)));
                }
                break;
            }
            DocumentNode::Object(kept)
        }
        DocumentNode::Array(items) => {
            let mut kept = Vec::new();
            let mut used = 2usize;
            for item in items {
                let size = decoded_size(&item);
                if used + size <= budget {
                    used += size;
                    kept.push(item);
                    continue;
                }
                let room = budget.saturating_sub(used);
                if room >= SCALAR_BYTES {
                    kept.push(shrink(item, room));
                }
                break;
            }
            DocumentNode::Array(kept)
        }
        DocumentNode::Text(text) if text.len() + 2 > budget => DocumentNode::Opaque {
            type_name: "string".into(),
            len: text.chars().count(),
        },
        other => other,
    }
}

pub fn cut_texts(
    documents: &mut [DocumentNode],
    offset: usize,
    max_chars: usize,
) -> Option<NodePosition> {
    let mut first = None;
    for (document, node) in documents.iter_mut().enumerate() {
        cut_node(
            node,
            offset,
            max_chars,
            &mut Vec::new(),
            document,
            &mut first,
        );
    }
    first
}

fn cut_node(
    node: &mut DocumentNode,
    offset: usize,
    max_chars: usize,
    steps: &mut Vec<usize>,
    document: usize,
    first: &mut Option<NodePosition>,
) {
    match node {
        DocumentNode::Text(text) => {
            let remaining = text.chars().count().saturating_sub(offset);
            let kept: String = text.chars().skip(offset).take(max_chars).collect();
            if remaining > max_chars {
                *text = format!("{kept}…[+{} chars]", remaining - max_chars);
                first.get_or_insert_with(|| NodePosition {
                    document,
                    steps: steps.clone(),
                });
            } else {
                *text = kept;
            }
        }
        DocumentNode::Array(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                steps.push(index);
                cut_node(item, offset, max_chars, steps, document, first);
                steps.pop();
            }
        }
        DocumentNode::Object(entries) => {
            for (index, (_, value)) in entries.iter_mut().enumerate() {
                steps.push(index);
                cut_node(value, offset, max_chars, steps, document, first);
                steps.pop();
            }
        }
        _ => {}
    }
}

pub fn fit_documents(documents: Vec<Value>, max_bytes: usize) -> (Vec<Value>, bool) {
    let mut kept = Vec::with_capacity(documents.len());
    let mut used = 2usize;
    for document in documents {
        let size = serialized_len(&document) + 1;
        if used + size <= max_bytes {
            used += size;
            kept.push(document);
            continue;
        }
        if kept.is_empty() {
            kept.push(shrink_value(document, max_bytes.saturating_sub(used)));
        }
        return (kept, true);
    }
    (kept, false)
}

fn shrink_value(value: Value, budget: usize) -> Value {
    match value {
        Value::Object(entries) => {
            let mut kept = Map::new();
            let mut used = 2usize;
            for (key, item) in entries {
                let overhead = serialized_len(&Value::String(key.clone())) + 2;
                let size = overhead + serialized_len(&item);
                if used + size <= budget {
                    used += size;
                    kept.insert(key, item);
                    continue;
                }
                let room = budget.saturating_sub(used + overhead);
                if matches!(item, Value::Object(_) | Value::Array(_)) && room > 2 {
                    kept.insert(key, shrink_value(item, room));
                }
                break;
            }
            Value::Object(kept)
        }
        Value::Array(items) => {
            let mut kept = Vec::new();
            let mut used = 2usize;
            for item in items {
                let size = serialized_len(&item) + 1;
                if used + size <= budget {
                    used += size;
                    kept.push(item);
                    continue;
                }
                let room = budget.saturating_sub(used);
                if matches!(item, Value::Object(_) | Value::Array(_)) && room > 2 {
                    kept.push(shrink_value(item, room));
                }
                break;
            }
            Value::Array(kept)
        }
        _ => Value::Null,
    }
}

fn serialized_len(value: &Value) -> usize {
    serde_json::to_string(value).map_or(0, |text| text.len())
}

#[cfg(test)]
mod unit {
    use serde_json::json;

    use super::*;

    fn text(value: &str) -> DocumentNode {
        DocumentNode::Text(value.into())
    }

    #[test]
    fn un_tableau_trop_long_est_coupe_et_sa_position_retrouve_le_chemin_affiche() {
        let mut documents = vec![DocumentNode::Object(vec![
            ("_id".into(), DocumentNode::Int(1)),
            (
                "items".into(),
                DocumentNode::Array((0..300).map(DocumentNode::Int).collect()),
            ),
        ])];
        let cut = cap_arrays(&mut documents, 100);
        assert_eq!(cut.len(), 1);
        let DocumentNode::Object(entries) = &documents[0] else {
            panic!("object");
        };
        let DocumentNode::Array(items) = &entries[1].1 else {
            panic!("array");
        };
        assert_eq!(items.len(), 100);
        assert_eq!(shown_path(&documents, &cut[0]), "items");
    }

    #[test]
    fn un_texte_long_est_coupe_a_partir_du_decalage() {
        let long: String = "a".repeat(2_000);
        let mut documents = vec![DocumentNode::Object(vec![("note".into(), text(&long))])];
        let cut = cut_texts(&mut documents, 500, 500).expect("cut");
        assert_eq!(shown_path(&documents, &cut), "note");
        let DocumentNode::Object(entries) = &documents[0] else {
            panic!("object");
        };
        assert_eq!(
            entries[0].1,
            text(&format!("{}…[+1000 chars]", "a".repeat(500)))
        );
    }

    #[test]
    fn un_seul_document_trop_gros_est_reduit_a_ce_qui_tient() {
        let document = json!({"_id": 1, "items": (0..5_000).collect::<Vec<_>>()});
        let (kept, cut) = fit_documents(vec![document, json!({"_id": 2})], 1_024);
        assert!(cut);
        assert_eq!(kept.len(), 1);
        assert!(serde_json::to_string(&kept).unwrap().len() <= 1_024);
        assert_eq!(kept[0]["_id"], 1);
    }

    #[test]
    fn les_documents_sont_gardes_tant_que_le_budget_tient() {
        let documents: Vec<Value> = (0..100).map(|id| json!({"_id": id})).collect();
        let (kept, cut) = fit_documents(documents, 200);
        assert!(cut);
        assert!(serde_json::to_string(&kept).unwrap().len() <= 200);
        assert!(kept.len() > 1);
    }

    #[test]
    fn un_document_decode_trop_gros_est_reduit_et_un_texte_qui_ne_tient_pas_devient_opaque() {
        let document = DocumentNode::Object(vec![
            ("_id".into(), DocumentNode::Int(1)),
            ("note".into(), text(&"x".repeat(10_000))),
        ]);
        let (kept, cut) = prune_decoded(vec![document], 1_000);
        assert!(cut);
        let DocumentNode::Object(entries) = &kept[0] else {
            panic!("object");
        };
        assert_eq!(entries[0].1, DocumentNode::Int(1));
        assert_eq!(
            entries[1].1,
            DocumentNode::Opaque {
                type_name: "string".into(),
                len: 10_000
            }
        );
    }
}
