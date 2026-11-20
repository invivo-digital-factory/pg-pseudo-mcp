use serde_json::Value;

use super::anonymization_mapping::{cell, column, lineage, message_in, message_out, rows_out};
use crate::contexts::anonymization::injector::AnonymizationUseCases;
use crate::contexts::querying::domain::repositories::ResultAnonymizer;
use crate::contexts::querying::domain::types::{
    AnonymizedRows, Cell, ColumnMeta, DbMessage, Lineage,
};

pub struct ResultAnonymizerAdapter {
    use_cases: AnonymizationUseCases,
}

impl ResultAnonymizerAdapter {
    pub fn new(use_cases: AnonymizationUseCases) -> Self {
        Self { use_cases }
    }
}

impl ResultAnonymizer for ResultAnonymizerAdapter {
    fn rows(
        &self,
        columns: &[ColumnMeta],
        rows: Vec<Vec<Cell>>,
        lineage_per_column: Option<&[Lineage]>,
        injected: &[(String, String)],
    ) -> AnonymizedRows {
        let columns: Vec<_> = columns.iter().map(column).collect();
        let rows = rows
            .into_iter()
            .map(|row| row.into_iter().map(cell).collect())
            .collect();
        let lineage_per_column: Option<Vec<_>> =
            lineage_per_column.map(|items| items.iter().copied().map(lineage).collect());
        rows_out(self.use_cases.anonymize_result_set.execute(
            &columns,
            rows,
            lineage_per_column.as_deref(),
            injected,
        ))
    }

    fn free_text(&self, text: &str, injected: &[(String, String)]) -> String {
        self.use_cases.anonymize_free_text.execute(text, injected)
    }

    fn message(&self, message: &DbMessage, injected: &[(String, String)]) -> DbMessage {
        message_out(
            self.use_cases
                .anonymize_message
                .execute(&message_in(message), injected),
        )
    }

    fn plan(&self, plan: Value, injected: &[(String, String)]) -> Value {
        self.use_cases.anonymize_plan.execute(plan, injected)
    }
}

#[cfg(test)]
mod integration {
    use super::*;
    use crate::contexts::anonymization::injector::AnonymizationInjector;

    #[test]
    fn les_chaines_d_un_plan_json_sont_nettoyees_avant_l_echappement() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = ResultAnonymizerAdapter::new(
            AnonymizationInjector::new(dir.path()).use_cases().unwrap(),
        );
        let real = "o\"brien@canary-mail.fr".to_string();
        let injected = vec![(real.clone(), "anne.lorin7@example.net".to_string())];
        let plan =
            serde_json::json!([{ "Plan": { "Filter": format!("(email = '{real}'::text)") } }]);
        let text = serde_json::to_string_pretty(&adapter.plan(plan, &injected)).unwrap();
        assert!(text.contains("anne.lorin7@example.net"), "{text}");
        assert!(!text.contains("brien"), "{text}");
    }
}
