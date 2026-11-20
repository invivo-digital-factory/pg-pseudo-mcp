pub enum StoreOutcome {
    Known(String),
    Created(String),
    Exhausted,
}
