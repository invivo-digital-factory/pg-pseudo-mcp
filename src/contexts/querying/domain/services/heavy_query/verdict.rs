use std::collections::HashMap;

use crate::contexts::querying::domain::constants::heavy_query::{MONGO_COLLSCAN, PG_SEQ_SCAN};
use crate::contexts::querying::domain::types::{
    CollectionScan, FullScan, HeavyQueryEstimate, HeavyQueryThresholds, PlanSummary,
};

pub fn pg_verdict(
    summary: &PlanSummary,
    relation_rows: &HashMap<String, f64>,
    thresholds: HeavyQueryThresholds,
) -> Option<HeavyQueryEstimate> {
    let full_scans: Vec<FullScan> = summary
        .scans
        .iter()
        .filter_map(|scan| {
            let rows = relation_rows
                .get(&scan.qualified_name())
                .copied()
                .filter(|rows| *rows >= 0.0)
                .unwrap_or(scan.plan_rows);
            let scanned = rows * scan.limit_fraction.unwrap_or(1.0);
            (scanned > thresholds.scan_limit as f64).then(|| FullScan {
                node: PG_SEQ_SCAN,
                relation: scan.qualified_name(),
                estimated_rows: scanned.round() as u64,
            })
        })
        .collect();
    let too_costly = summary.total_cost > thresholds.cost_limit as f64;
    (too_costly || !full_scans.is_empty()).then_some(HeavyQueryEstimate {
        total_cost: Some(summary.total_cost),
        cost_limit: Some(thresholds.cost_limit),
        scan_limit: thresholds.scan_limit,
        full_scans,
    })
}

pub fn mongo_verdict(
    scans: &[CollectionScan],
    estimated_counts: &HashMap<String, u64>,
    thresholds: HeavyQueryThresholds,
) -> Option<HeavyQueryEstimate> {
    let mut full_scans: Vec<FullScan> = Vec::new();
    for scan in scans.iter().filter(|scan| needs_count(scan)) {
        let Some(count) = estimated_counts.get(&scan.collection).copied() else {
            continue;
        };
        let already = full_scans
            .iter()
            .any(|known| known.relation == scan.collection);
        if count > thresholds.scan_limit && !already {
            full_scans.push(FullScan {
                node: MONGO_COLLSCAN,
                relation: scan.collection.clone(),
                estimated_rows: count,
            });
        }
    }
    (!full_scans.is_empty()).then_some(HeavyQueryEstimate {
        total_cost: None,
        cost_limit: None,
        scan_limit: thresholds.scan_limit,
        full_scans,
    })
}

pub fn needs_count(scan: &CollectionScan) -> bool {
    scan.filtered || !scan.stopped_by_limit
}
