#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HeavyQueryLimits {
    pub cost: Option<u64>,
    pub scan_rows: Option<u64>,
}
