use rmcp::model::CallToolResult;

use super::error_mapping::wrong_engine;
use crate::contexts::querying::domain::constants::mongo_policy::{
    USE_DOCUMENT_TOOLS, USE_SQL_TOOLS,
};

pub fn use_document_tools() -> CallToolResult {
    wrong_engine(USE_DOCUMENT_TOOLS).into_call_result()
}

pub fn use_sql_tools() -> CallToolResult {
    wrong_engine(USE_SQL_TOOLS).into_call_result()
}
