use crate::contexts::connections::domain::types::{ConnectionEntry, ConnectionType};
use crate::contexts::connections::injector::ConnectionsContext;
use crate::contexts::querying::domain::types::HeavyQueryThresholds;

pub async fn heavy_query_thresholds(
    connections: &ConnectionsContext,
    connection: &str,
) -> Option<HeavyQueryThresholds> {
    let discovery = connections.list_connections.execute().await.ok()?;
    discovery
        .connections
        .iter()
        .find(|entry| entry.name == connection)
        .and_then(thresholds_of)
}

pub fn thresholds_of(entry: &ConnectionEntry) -> Option<HeavyQueryThresholds> {
    (entry.connection_type == ConnectionType::Prod.as_str()).then(|| {
        HeavyQueryThresholds::with_overrides(entry.heavy_query.cost, entry.heavy_query.scan_rows)
    })
}
