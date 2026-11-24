use std::future::Future;

use crate::contexts::querying::domain::types::Engine;

pub trait ConnectionEngines: Send + Sync {
    fn engine_of(&self, connection: &str) -> impl Future<Output = Option<Engine>> + Send;
}
