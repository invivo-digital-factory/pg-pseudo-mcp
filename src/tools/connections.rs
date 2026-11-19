use rmcp::model::CallToolResult;

use crate::common::AppConfig;
use crate::contexts::connections::injector::ConnectionsContext;
use crate::contexts::connections::presentation;

pub async fn list_connections(config: &AppConfig) -> CallToolResult {
    presentation::list_connections(&ConnectionsContext::new(config).list_connections).await
}
