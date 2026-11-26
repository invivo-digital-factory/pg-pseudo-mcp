use serde_json::Value;

use crate::contexts::querying::domain::constants::heavy_query::{
    PG_LIMIT, PG_PASS_THROUGH, PG_SEQ_SCAN,
};
use crate::contexts::querying::domain::types::{PlanScan, PlanSummary};

pub fn summarize_pg_plan(plan: &Value) -> Option<PlanSummary> {
    let root = plan
        .as_array()
        .and_then(|entries| entries.first())
        .unwrap_or(plan)
        .get("Plan")?;
    let total_cost = root.get("Total Cost")?.as_f64()?;
    let mut scans = Vec::new();
    collect_scans(root, None, &mut scans);
    Some(PlanSummary { total_cost, scans })
}

fn collect_scans(node: &Value, limit_rows: Option<f64>, scans: &mut Vec<PlanScan>) {
    let node_type = node.get("Node Type").and_then(Value::as_str).unwrap_or("");
    let plan_rows = node.get("Plan Rows").and_then(Value::as_f64).unwrap_or(0.0);
    if node_type == PG_SEQ_SCAN
        && let Some(relation) = node.get("Relation Name").and_then(Value::as_str)
    {
        scans.push(PlanScan {
            schema: node
                .get("Schema")
                .and_then(Value::as_str)
                .map(str::to_string),
            relation: relation.to_string(),
            plan_rows,
            limit_fraction: limit_rows.map(|limit| (limit / plan_rows.max(1.0)).min(1.0)),
        });
    }
    let child_limit = if node_type == PG_LIMIT {
        Some(plan_rows)
    } else if PG_PASS_THROUGH.contains(&node_type) {
        limit_rows
    } else {
        None
    };
    for child in node
        .get("Plans")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        collect_scans(child, child_limit, scans);
    }
}
