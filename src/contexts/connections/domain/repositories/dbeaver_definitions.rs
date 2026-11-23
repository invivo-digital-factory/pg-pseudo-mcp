use crate::contexts::connections::domain::types::DbeaverRead;

pub trait DbeaverDefinitions: Send + Sync {
    fn read(&self) -> Result<DbeaverRead, String>;
}
