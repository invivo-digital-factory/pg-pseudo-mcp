use crate::contexts::anonymization::domain::errors::PseudoError;

impl From<rusqlite::Error> for PseudoError {
    fn from(error: rusqlite::Error) -> Self {
        PseudoError::Store(error.to_string())
    }
}
