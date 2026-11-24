use std::sync::Arc;

use crate::contexts::anonymization::domain::types::Kind;
use crate::contexts::connections::domain::types as connections;
use crate::contexts::connections::injector::ConnectionsContext;
use crate::contexts::querying::domain::repositories::ColumnPolicySource;
use crate::contexts::querying::domain::services::column_policy::merge_policy;
use crate::contexts::querying::domain::types::{
    ColumnPolicyDocument, ColumnPolicyEntries, ColumnPolicyLoad, MongoPolicyEntries,
};

pub struct ColumnPolicySourceAdapter {
    connections: Arc<ConnectionsContext>,
}

impl ColumnPolicySourceAdapter {
    pub fn new(connections: Arc<ConnectionsContext>) -> Self {
        Self { connections }
    }
}

impl ColumnPolicySource for ColumnPolicySourceAdapter {
    fn load(&self) -> ColumnPolicyLoad {
        let documents: Vec<ColumnPolicyDocument> = self
            .connections
            .read_column_policy_files
            .execute()
            .into_iter()
            .map(document)
            .collect();
        merge_policy(&documents, &|kind| Kind::parse(kind).is_some())
    }
}

fn document(file: connections::ColumnPolicyFile) -> ColumnPolicyDocument {
    ColumnPolicyDocument {
        file: file.file,
        content: file.content.map(|entries| ColumnPolicyEntries {
            clear: entries.clear,
            personal: entries.personal,
            mongodb: MongoPolicyEntries {
                clear: entries.mongodb.clear,
                personal: entries.mongodb.personal,
            },
        }),
    }
}
