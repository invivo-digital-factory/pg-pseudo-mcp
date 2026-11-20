use serde_json::Value;

use crate::contexts::querying::domain::types::{ColumnMeta, QueryLimits, TruncatedBy, Truncation};

pub fn bound_rows(
    rows: Vec<Vec<Value>>,
    columns: &[ColumnMeta],
    limits: &QueryLimits,
    cell_offset: usize,
    truncated: &mut Vec<Truncation>,
) -> Vec<Vec<Value>> {
    let mut kept = Vec::with_capacity(rows.len());
    let mut bytes = 0usize;
    let mut cut_column: Option<String> = None;
    for mut row in rows {
        for (index, value) in row.iter_mut().enumerate() {
            if cut_cell(value, cell_offset, limits.max_cell_chars) && cut_column.is_none() {
                cut_column = columns.get(index).map(|column| column.name.clone());
            }
        }
        let size = serde_json::to_string(&row).map_or(0, |text| text.len());
        if bytes + size > limits.max_bytes && !kept.is_empty() {
            truncated.push(Truncation {
                by: TruncatedBy::Bytes,
                limit: limits.max_bytes as u64,
                column: None,
                hint: "select fewer or narrower columns, or fetch fewer rows".into(),
            });
            break;
        }
        bytes += size;
        kept.push(row);
    }
    if let Some(column) = cut_column {
        truncated.push(Truncation {
            by: TruncatedBy::Cell,
            limit: limits.max_cell_chars as u64,
            column: Some(column),
            hint: format!(
                "long values are cut; repeat the same query with cell_offset={} to read the rest",
                cell_offset.saturating_add(limits.max_cell_chars)
            ),
        });
    }
    kept
}

fn cut_cell(value: &mut Value, offset: usize, max_chars: usize) -> bool {
    match value {
        Value::String(text) => {
            let remaining = text.chars().count().saturating_sub(offset);
            let kept: String = text.chars().skip(offset).take(max_chars).collect();
            *text = if remaining > max_chars {
                format!("{kept}…[+{} chars]", remaining - max_chars)
            } else {
                kept
            };
            remaining > max_chars
        }
        Value::Array(items) => {
            let mut cut = false;
            for item in items.iter_mut() {
                cut |= cut_cell(item, offset, max_chars);
            }
            cut
        }
        Value::Object(entries) => {
            let mut cut = false;
            for item in entries.values_mut() {
                cut |= cut_cell(item, offset, max_chars);
            }
            cut
        }
        _ => false,
    }
}

#[cfg(test)]
mod unit {
    use serde_json::json;

    use super::*;

    fn column(name: &str) -> ColumnMeta {
        ColumnMeta {
            name: name.into(),
            type_name: "text".into(),
            source: None,
        }
    }

    #[test]
    fn un_decalage_lit_la_tranche_suivante_de_chaque_cellule_texte() {
        let long: String = ('a'..='z').cycle().take(1200).collect();
        let rows = vec![vec![json!(long), json!("short"), json!(7)]];
        let columns = [column("body"), column("label"), column("id")];
        let mut truncated = Vec::new();
        let kept = bound_rows(rows, &columns, &QueryLimits::default(), 500, &mut truncated);
        let expected: String = long.chars().skip(500).take(500).collect();
        assert_eq!(kept[0][0], json!(format!("{expected}…[+200 chars]")));
        assert_eq!(kept[0][1], json!(""));
        assert_eq!(kept[0][2], json!(7));
        assert_eq!(truncated.len(), 1);
        assert_eq!(truncated[0].column.as_deref(), Some("body"));
        assert!(truncated[0].hint.contains("cell_offset=1000"));
        assert!(!truncated[0].hint.contains("substr"));
    }

    #[test]
    fn la_derniere_tranche_n_est_pas_signalee_comme_tronquee() {
        let long: String = "x".repeat(1200);
        let mut truncated = Vec::new();
        let kept = bound_rows(
            vec![vec![json!(long)]],
            &[column("body")],
            &QueryLimits::default(),
            1000,
            &mut truncated,
        );
        assert_eq!(kept[0][0], json!("x".repeat(200)));
        assert!(truncated.is_empty());
    }
}
