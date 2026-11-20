use crate::contexts::connections::domain::types::Discovery;

pub trait ConnectionCatalog: Send + Sync {
    fn discover_in(&self, database: Option<&str>) -> Discovery;

    fn discover(&self) -> Discovery {
        self.discover_in(None)
    }
}
