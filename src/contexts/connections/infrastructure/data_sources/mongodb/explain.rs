use mongodb::Client;
use mongodb::bson::{Bson, Document, doc};
use mongodb::options::SelectionCriteria;

use super::failure::MongoFailure;

pub const QUERY_PLANNER: &str = "queryPlanner";

pub async fn query_plan(
    client: &Client,
    database: &str,
    command: Document,
    read_preference: SelectionCriteria,
) -> Result<serde_json::Value, MongoFailure> {
    let explained = client
        .database(database)
        .run_command(doc! {"explain": command, "verbosity": QUERY_PLANNER})
        .selection_criteria(read_preference)
        .await?;
    Ok(Bson::Document(explained).into_relaxed_extjson())
}
