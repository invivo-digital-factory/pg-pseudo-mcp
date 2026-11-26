mod mongo_plan;
mod pg_plan;
mod relation_rows;
mod verdict;

pub use mongo_plan::collection_scans;
pub use pg_plan::summarize_pg_plan;
pub use relation_rows::{relation_rows, relation_rows_sql};
pub use verdict::{mongo_verdict, needs_count, pg_verdict};

pub fn explain_for_cost(sql: &str) -> String {
    format!("EXPLAIN (VERBOSE, FORMAT JSON) {sql}")
}
