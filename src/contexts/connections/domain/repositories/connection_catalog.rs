use crate::contexts::connections::domain::types::Discovery;

pub trait ConnectionCatalog: Send + Sync {
    fn discover(&self) -> Discovery;
}
