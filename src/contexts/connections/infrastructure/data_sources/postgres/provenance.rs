use std::collections::HashMap;

use tokio_postgres::{Column, Transaction};

use crate::contexts::connections::domain::types::ColumnSource;

const LOOKUP: &str = "SELECT a.attrelid, a.attnum, n.nspname::text, c.relname::text, a.attname::text \
     FROM pg_catalog.pg_attribute a \
     JOIN pg_catalog.pg_class c ON c.oid = a.attrelid \
     JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
     WHERE (a.attrelid, a.attnum) IN \
     (SELECT * FROM ROWS FROM (pg_catalog.unnest($1::pg_catalog.oid[]), pg_catalog.unnest($2::pg_catalog.int2[])))";

type Origin = (u32, i16);

#[derive(Default)]
pub(crate) struct SourceCache {
    known: HashMap<Origin, Option<ColumnSource>>,
}

impl SourceCache {
    pub(crate) async fn resolve(
        &mut self,
        transaction: &Transaction<'_>,
        columns: &[Column],
    ) -> Result<Vec<Option<ColumnSource>>, tokio_postgres::Error> {
        let origins: Vec<Option<Origin>> = columns.iter().map(origin).collect();
        let mut missing: Vec<Origin> = origins
            .iter()
            .flatten()
            .filter(|origin| !self.known.contains_key(origin))
            .copied()
            .collect();
        missing.sort_unstable();
        missing.dedup();
        if !missing.is_empty() {
            let tables: Vec<u32> = missing.iter().map(|(table, _)| *table).collect();
            let attributes: Vec<i16> = missing.iter().map(|(_, attribute)| *attribute).collect();
            let rows = transaction.query(LOOKUP, &[&tables, &attributes]).await?;
            for origin in missing {
                self.known.insert(origin, None);
            }
            for row in rows {
                self.known.insert(
                    (row.try_get(0)?, row.try_get(1)?),
                    Some(ColumnSource {
                        schema: row.try_get(2)?,
                        table: row.try_get(3)?,
                        column: row.try_get(4)?,
                    }),
                );
            }
        }
        Ok(origins
            .into_iter()
            .map(|origin| origin.and_then(|origin| self.known.get(&origin).cloned().flatten()))
            .collect())
    }
}

fn origin(column: &Column) -> Option<Origin> {
    match (column.table_oid(), column.column_id()) {
        (Some(table), Some(attribute)) if table != 0 && attribute > 0 => Some((table, attribute)),
        _ => None,
    }
}
