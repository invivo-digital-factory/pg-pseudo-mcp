use std::time::Duration;

use futures_util::TryStreamExt;
use mongodb::bson::{Bson, Document, doc};
use mongodb::options::{ClientOptions, Collation, Hint, ReadPreference, SelectionCriteria};
use mongodb::{Client, Collection, Cursor};

use super::catalog::{collection_kind, index_keys, validator_fields};
use super::decode::decode_document;
use super::extended_json::{bson_from_extended_json, document_from_extended_json};
use super::failure::MongoFailure;
use super::sample::{SAMPLE_MAX_TIME, sample_pipeline, sampled_fields};
use crate::contexts::connections::domain::services::is_listed_collection;
use crate::contexts::connections::domain::types::{
    AggregateRequest, CollectionDescription, CollectionInfo, CollectionKind, DocumentOutput,
    ExecutionLimits, FindRequest, SampledField,
};

const COMMENT: &str = "scarif";
const METADATA_MAX_TIME: Duration = Duration::from_secs(5);

pub struct MongoSession {
    client: Client,
}

impl MongoSession {
    pub fn open(options: ClientOptions) -> Result<Self, MongoFailure> {
        Ok(Self {
            client: Client::with_options(options)?,
        })
    }

    pub async fn find(
        &self,
        database: &str,
        request: &FindRequest,
        limits: ExecutionLimits,
    ) -> Result<DocumentOutput, MongoFailure> {
        let fetched = fetched(limits);
        let limit = request
            .limit
            .filter(|limit| *limit > 0)
            .map_or(fetched, |limit| limit.min(fetched));
        let collection = self.collection(database, &request.collection);
        let mut find = collection
            .find(document_from_extended_json(&request.filter, "filter")?)
            .limit(i64::try_from(limit).unwrap_or(i64::MAX))
            .batch_size(u32::try_from(fetched).unwrap_or(u32::MAX))
            .max_time(Duration::from_millis(limits.timeout_ms))
            .allow_disk_use(false)
            .selection_criteria(secondary_preferred())
            .comment(Bson::String(COMMENT.to_string()));
        if let Some(projection) = &request.projection {
            find = find.projection(document_from_extended_json(projection, "projection")?);
        }
        if let Some(sort) = &request.sort {
            find = find.sort(document_from_extended_json(sort, "sort")?);
        }
        if let Some(skip) = request.skip {
            find = find.skip(skip);
        }
        if let Some(hint) = &request.hint {
            find = find.hint(hint_of(hint)?);
        }
        if let Some(collation) = &request.collation {
            find = find.collation(collation_of(collation)?);
        }
        bounded(find.await?, limits.max_rows).await
    }

    pub async fn aggregate(
        &self,
        database: &str,
        request: &AggregateRequest,
        limits: ExecutionLimits,
    ) -> Result<DocumentOutput, MongoFailure> {
        let fetched = fetched(limits);
        let mut pipeline = request
            .pipeline
            .iter()
            .enumerate()
            .map(|(index, stage)| {
                document_from_extended_json(stage, &format!("pipeline stage {}", index + 1))
            })
            .collect::<Result<Vec<_>, _>>()?;
        pipeline.push(doc! {"$limit": i64::try_from(fetched).unwrap_or(i64::MAX)});
        let collection = self.collection(database, &request.collection);
        let mut aggregate = collection
            .aggregate(pipeline)
            .batch_size(u32::try_from(fetched).unwrap_or(u32::MAX))
            .max_time(Duration::from_millis(limits.timeout_ms))
            .allow_disk_use(false)
            .selection_criteria(secondary_preferred())
            .comment(Bson::String(COMMENT.to_string()));
        if let Some(variables) = &request.let_vars {
            aggregate = aggregate.let_vars(document_from_extended_json(variables, "let")?);
        }
        bounded(aggregate.await?, limits.max_rows).await
    }

    pub async fn database_names(&self) -> Result<Vec<String>, MongoFailure> {
        Ok(self
            .client
            .list_database_names()
            .authorized_databases(true)
            .await?)
    }

    pub async fn collections(&self, database: &str) -> Result<Vec<CollectionInfo>, MongoFailure> {
        let specifications: Vec<_> = self
            .client
            .database(database)
            .list_collections()
            .await?
            .try_collect()
            .await?;
        let mut collections = Vec::new();
        for specification in specifications {
            if !is_listed_collection(&specification.name) {
                continue;
            }
            let kind = collection_kind(&specification.collection_type);
            let estimated_count = self
                .estimated_count(database, &specification.name, kind)
                .await;
            collections.push(CollectionInfo {
                name: specification.name,
                kind,
                estimated_count,
            });
        }
        collections.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(collections)
    }

    pub async fn describe(
        &self,
        database: &str,
        collection: &str,
    ) -> Result<Option<CollectionDescription>, MongoFailure> {
        let specification = self
            .client
            .database(database)
            .list_collections()
            .filter(doc! {"name": collection})
            .await?
            .try_next()
            .await?;
        let Some(specification) = specification else {
            return Ok(None);
        };
        let kind = collection_kind(&specification.collection_type);
        let validator = specification
            .options
            .validator
            .as_ref()
            .map(validator_fields)
            .unwrap_or_default();
        let indexes = if kind == CollectionKind::View {
            Vec::new()
        } else {
            let models: Vec<_> = self
                .collection(database, collection)
                .list_indexes()
                .max_time(METADATA_MAX_TIME)
                .await?
                .try_collect()
                .await?;
            models.iter().map(|model| index_keys(&model.keys)).collect()
        };
        Ok(Some(CollectionDescription {
            estimated_count: self.estimated_count(database, collection, kind).await,
            indexes,
            validator_fields: validator,
        }))
    }

    pub async fn sample(
        &self,
        database: &str,
        collection: &str,
    ) -> Result<Vec<SampledField>, MongoFailure> {
        let documents: Vec<Document> = self
            .collection(database, collection)
            .aggregate(sample_pipeline())
            .max_time(SAMPLE_MAX_TIME)
            .allow_disk_use(false)
            .selection_criteria(secondary_preferred())
            .comment(Bson::String(COMMENT.to_string()))
            .await?
            .try_collect()
            .await?;
        Ok(sampled_fields(&documents))
    }

    async fn estimated_count(
        &self,
        database: &str,
        collection: &str,
        kind: CollectionKind,
    ) -> Option<u64> {
        if kind != CollectionKind::Collection {
            return None;
        }
        self.collection(database, collection)
            .estimated_document_count()
            .max_time(METADATA_MAX_TIME)
            .selection_criteria(secondary_preferred())
            .await
            .ok()
    }

    fn collection(&self, database: &str, collection: &str) -> Collection<Document> {
        self.client.database(database).collection(collection)
    }
}

fn fetched(limits: ExecutionLimits) -> u64 {
    u64::try_from(limits.max_rows)
        .unwrap_or(u64::MAX)
        .saturating_add(1)
}

fn secondary_preferred() -> SelectionCriteria {
    SelectionCriteria::ReadPreference(ReadPreference::SecondaryPreferred { options: None })
}

fn hint_of(value: &serde_json::Value) -> Result<Hint, MongoFailure> {
    match bson_from_extended_json(value, "hint")? {
        Bson::String(name) => Ok(Hint::Name(name)),
        Bson::Document(keys) => Ok(Hint::Keys(keys)),
        _ => Err(MongoFailure::InvalidRequest(
            "the hint must be an index name or a key pattern".to_string(),
        )),
    }
}

fn collation_of(value: &serde_json::Value) -> Result<Collation, MongoFailure> {
    let document = document_from_extended_json(value, "collation")?;
    mongodb::bson::from_document(document)
        .map_err(|_| MongoFailure::InvalidRequest("the collation is not valid".to_string()))
}

async fn bounded(
    mut cursor: Cursor<Document>,
    max_rows: usize,
) -> Result<DocumentOutput, MongoFailure> {
    let mut documents = Vec::new();
    let mut more_documents = false;
    while let Some(document) = cursor.try_next().await? {
        if documents.len() == max_rows {
            more_documents = true;
            break;
        }
        documents.push(decode_document(document));
    }
    drop(cursor);
    Ok(DocumentOutput {
        documents,
        more_documents,
    })
}
