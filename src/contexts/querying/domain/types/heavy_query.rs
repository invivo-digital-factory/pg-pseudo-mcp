use serde::Serialize;

use crate::contexts::querying::domain::constants::heavy_query::{
    DEFAULT_COST_LIMIT, DEFAULT_SCAN_LIMIT,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HeavyQueryThresholds {
    pub cost_limit: u64,
    pub scan_limit: u64,
}

impl HeavyQueryThresholds {
    pub fn with_overrides(cost_limit: Option<u64>, scan_limit: Option<u64>) -> Self {
        Self {
            cost_limit: cost_limit.unwrap_or(DEFAULT_COST_LIMIT),
            scan_limit: scan_limit.unwrap_or(DEFAULT_SCAN_LIMIT),
        }
    }
}

impl Default for HeavyQueryThresholds {
    fn default() -> Self {
        Self::with_overrides(None, None)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanScan {
    pub schema: Option<String>,
    pub relation: String,
    pub plan_rows: f64,
    pub limit_fraction: Option<f64>,
}

impl PlanScan {
    pub fn qualified_name(&self) -> String {
        match &self.schema {
            Some(schema) => format!("{schema}.{}", self.relation),
            None => self.relation.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanSummary {
    pub total_cost: f64,
    pub scans: Vec<PlanScan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionScan {
    pub collection: String,
    pub filtered: bool,
    pub stopped_by_limit: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FullScan {
    pub node: &'static str,
    pub relation: String,
    pub estimated_rows: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HeavyQueryEstimate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_cost: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_limit: Option<u64>,
    pub scan_limit: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub full_scans: Vec<FullScan>,
}
