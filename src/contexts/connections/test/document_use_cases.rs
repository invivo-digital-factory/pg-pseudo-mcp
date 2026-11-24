use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};

use crate::contexts::connections::domain::constants::AGGREGATE_WRITE_STAGE_REFUSED;
use crate::contexts::connections::domain::errors::ConnectionError;
use crate::contexts::connections::domain::repositories::DocumentSessionPool;
use crate::contexts::connections::domain::types::{
    AggregateRequest, CollectionDescription, CollectionInfo, DatabaseNames, DocumentOutput,
    ExecutionLimits, FindRequest, SampledField,
};
use crate::contexts::connections::use_cases::RunAggregate;

#[derive(Default)]
struct RecordingPool {
    calls: AtomicUsize,
}

impl RecordingPool {
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn record(&self) {
        self.calls.fetch_add(1, Ordering::SeqCst);
    }
}

fn empty_output() -> DocumentOutput {
    DocumentOutput {
        documents: Vec::new(),
        more_documents: false,
    }
}

impl DocumentSessionPool for RecordingPool {
    async fn find(
        &self,
        _: &str,
        _: &FindRequest,
        _: ExecutionLimits,
    ) -> Result<DocumentOutput, ConnectionError> {
        self.record();
        Ok(empty_output())
    }

    async fn aggregate(
        &self,
        _: &str,
        _: &AggregateRequest,
        _: ExecutionLimits,
    ) -> Result<DocumentOutput, ConnectionError> {
        self.record();
        Ok(empty_output())
    }

    async fn database_names(&self, _: &str) -> Result<DatabaseNames, ConnectionError> {
        self.record();
        Ok(DatabaseNames {
            authorized: Vec::new(),
            default: None,
        })
    }

    async fn collections(
        &self,
        _: &str,
        _: Option<&str>,
    ) -> Result<Vec<CollectionInfo>, ConnectionError> {
        self.record();
        Ok(Vec::new())
    }

    async fn describe_collection(
        &self,
        _: &str,
        _: Option<&str>,
        _: &str,
    ) -> Result<CollectionDescription, ConnectionError> {
        self.record();
        Ok(CollectionDescription {
            estimated_count: None,
            indexes: Vec::new(),
            validator_fields: Vec::new(),
        })
    }

    async fn sample_fields(
        &self,
        _: &str,
        _: Option<&str>,
        _: &str,
    ) -> Result<Vec<SampledField>, ConnectionError> {
        self.record();
        Ok(Vec::new())
    }
}

fn limits() -> ExecutionLimits {
    ExecutionLimits {
        max_rows: 10,
        timeout_ms: 1_000,
    }
}

fn aggregate(pipeline: Vec<Value>) -> AggregateRequest {
    AggregateRequest {
        database: Some("shop".into()),
        collection: "orders".into(),
        pipeline,
        let_vars: None,
    }
}

async fn run(pipeline: Vec<Value>) -> (Result<DocumentOutput, ConnectionError>, usize) {
    let pool = Arc::new(RecordingPool::default());
    let outcome = RunAggregate::new(pool.clone())
        .execute("atlas", &aggregate(pipeline), limits())
        .await;
    (outcome, pool.calls())
}

#[tokio::test]
async fn un_pipeline_qui_ecrit_est_refuse_avant_tout_envoi_au_serveur_quelle_que_soit_sa_profondeur()
 {
    let writing = [
        vec![json!({"$match": {}}), json!({"$out": "copy"})],
        vec![json!({"$merge": {"into": "copy"}})],
        vec![json!({"$facet": {"a": [{"$match": {}}], "b": [{"$out": "copy"}]}})],
        vec![json!({"$lookup": {
            "from": "customers",
            "as": "c",
            "pipeline": [{"$merge": {"into": {"db": "other", "coll": "copy"}}}],
        }})],
        vec![json!({"$unionWith": {"coll": "customers", "pipeline": [{"$out": "copy"}]}})],
        vec![json!({"$facet": {"a": [{"$lookup": {
            "from": "customers",
            "as": "c",
            "pipeline": [{"$unionWith": {"coll": "x", "pipeline": [{"$out": "copy"}]}}],
        }}]}})],
    ];
    for pipeline in writing {
        let shown = format!("{pipeline:?}");
        let (outcome, calls) = run(pipeline).await;
        assert_eq!(
            outcome,
            Err(ConnectionError::Unsupported {
                reason: AGGREGATE_WRITE_STAGE_REFUSED.to_string(),
            }),
            "{shown}"
        );
        assert_eq!(calls, 0, "{shown}");
    }
}

#[tokio::test]
async fn un_pipeline_de_lecture_est_transmis_tel_quel() {
    let (outcome, calls) = run(vec![
        json!({"$match": {"status": "out"}}),
        json!({"$lookup": {"from": "customers", "as": "merge", "pipeline": [{"$match": {}}]}}),
        json!({"$unionWith": "archive"}),
        json!({"$project": {"out": 1, "merge": 1}}),
    ])
    .await;
    assert_eq!(outcome, Ok(empty_output()));
    assert_eq!(calls, 1);
}
