#[derive(Debug)]
pub enum MongoFailure {
    Driver(mongodb::error::Error),
    InvalidRequest(String),
}

impl From<mongodb::error::Error> for MongoFailure {
    fn from(error: mongodb::error::Error) -> Self {
        Self::Driver(error)
    }
}
