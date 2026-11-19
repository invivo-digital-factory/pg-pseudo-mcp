use std::sync::Arc;

use crate::contexts::querying::domain::constants::catalog::SUGGESTION_POOL;
use crate::contexts::querying::domain::errors::{DatabaseFailure, QueryingError, ScrubbedText};
use crate::contexts::querying::domain::repositories::{AnonymizationPorts, ReadOnlyDatabase};
use crate::contexts::querying::domain::services::catalog_queries::{known_tables_sql, resolve_sql};
use crate::contexts::querying::domain::services::catalog_reading::{candidates, known_tables};
use crate::contexts::querying::domain::services::comment_cap::cap_comment;
use crate::contexts::querying::domain::services::describe_sql::describe_sql;
use crate::contexts::querying::domain::services::description::{column_entry, read_description};
use crate::contexts::querying::domain::services::qualified_name::split_qualified;
use crate::contexts::querying::domain::services::relation::short_type;
use crate::contexts::querying::domain::services::table_picker::{Candidate, Pick, pick};
use crate::contexts::querying::domain::services::table_suggestions::closest_names;
use crate::contexts::querying::domain::types::{
    DescribeResponse, DescribeTableRequest, ExecutionLimits, QueryLimits, QueryOutput,
};

pub struct DescribeTable<D: ReadOnlyDatabase> {
    database: Arc<D>,
    anonymization: Result<AnonymizationPorts, String>,
}

impl<D: ReadOnlyDatabase> DescribeTable<D> {
    pub fn new(database: Arc<D>, anonymization: Result<AnonymizationPorts, String>) -> Self {
        Self {
            database,
            anonymization,
        }
    }

    pub async fn execute(
        &self,
        request: DescribeTableRequest,
    ) -> Result<DescribeResponse, QueryingError> {
        let ports =
            self.anonymization
                .as_ref()
                .map_err(|reason| QueryingError::StoreUnavailable {
                    reason: reason.clone(),
                })?;
        let connection = request.connection.as_str();
        let (schema, name) = split_qualified(&request.table);
        let target = self
            .resolve(ports, connection, &request.table, schema.as_deref(), &name)
            .await?;
        let output = self
            .fetch(ports, connection, &describe_sql(target.oid), 1)
            .await?;
        let row = output
            .rows
            .first()
            .ok_or_else(|| QueryingError::TableDisappeared {
                table: request.table.clone(),
            })?;
        let mut description = read_description(row);
        let comment = description
            .comment
            .as_deref()
            .map(|comment| self.scrubbed_comment(ports, comment));
        let columns = description
            .take_columns()
            .into_iter()
            .map(|column| {
                let type_name = short_type(&column.type_name);
                let class = ports.dictionary.classify(&column.name, &type_name);
                let column_comment = column
                    .comment
                    .as_deref()
                    .map(|comment| self.scrubbed_comment(ports, comment));
                column_entry(column, &class, column_comment)
            })
            .collect();
        Ok(description.into_response(comment, columns))
    }

    async fn fetch(
        &self,
        ports: &AnonymizationPorts,
        connection: &str,
        sql: &str,
        max_rows: usize,
    ) -> Result<QueryOutput, QueryingError> {
        let limits = QueryLimits::default();
        self.database
            .read_catalog(
                connection,
                sql,
                ExecutionLimits {
                    max_rows,
                    timeout_ms: limits.timeout_ms,
                },
            )
            .await
            .map_err(|failure| self.scrubbed(ports, failure, &limits))
    }

    async fn resolve(
        &self,
        ports: &AnonymizationPorts,
        connection: &str,
        requested: &str,
        schema: Option<&str>,
        name: &str,
    ) -> Result<Candidate, QueryingError> {
        let output = self
            .fetch(
                ports,
                connection,
                &resolve_sql(schema, name),
                SUGGESTION_POOL,
            )
            .await?;
        match pick(candidates(&output), schema, name) {
            Pick::One(candidate) => Ok(candidate),
            Pick::Ambiguous(names) => Err(QueryingError::AmbiguousTable {
                requested: requested.to_string(),
                candidates: names,
            }),
            Pick::None => Err(self.unknown_table(ports, connection, requested, name).await),
        }
    }

    async fn unknown_table(
        &self,
        ports: &AnonymizationPorts,
        connection: &str,
        requested: &str,
        name: &str,
    ) -> QueryingError {
        let known = match self
            .fetch(ports, connection, &known_tables_sql(), SUGGESTION_POOL)
            .await
        {
            Ok(output) => known_tables(&output),
            Err(error) => return error,
        };
        QueryingError::UnknownTable {
            requested: requested.to_string(),
            suggestions: closest_names(name, &known),
        }
    }

    fn scrubbed_comment(&self, ports: &AnonymizationPorts, comment: &str) -> String {
        cap_comment(ports.anonymizer.free_text(comment, &[]))
    }

    fn scrubbed(
        &self,
        ports: &AnonymizationPorts,
        failure: DatabaseFailure,
        limits: &QueryLimits,
    ) -> QueryingError {
        let scrubbed = match &failure {
            DatabaseFailure::Db(message) => {
                ScrubbedText::Message(ports.anonymizer.message(message, &[]))
            }
            DatabaseFailure::Connect(text) | DatabaseFailure::Decode(text) => {
                ScrubbedText::Text(ports.anonymizer.free_text(text, &[]))
            }
            _ => ScrubbedText::Nothing,
        };
        QueryingError::from_failure(failure, limits.timeout_ms, scrubbed)
    }
}
