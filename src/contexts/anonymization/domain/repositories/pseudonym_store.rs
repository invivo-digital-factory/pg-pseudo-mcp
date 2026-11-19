use crate::contexts::anonymization::domain::errors::PseudoError;
use crate::contexts::anonymization::domain::types::{Kind, StoreOutcome};

pub trait PseudonymStore: Send + Sync {
    fn get_or_create(
        &self,
        kind: Kind,
        forward_key: &[u8; 32],
        max_draws: usize,
        draw: &mut dyn FnMut() -> String,
        encrypt: &dyn Fn() -> Result<Vec<u8>, PseudoError>,
    ) -> Result<StoreOutcome, PseudoError>;

    fn find(&self, pseudonym: &str) -> Result<Vec<(String, Vec<u8>)>, PseudoError>;
}
