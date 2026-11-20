mod cli;

pub use cli::AzureCli;

use crate::contexts::connections::domain::errors::AccessTokenError;
use crate::contexts::connections::domain::types::Secret;

#[derive(Clone)]
pub struct FetchedToken {
    pub token: Secret,
    pub expires_at: u64,
}

pub trait AccessTokenFetcher: Send + Sync {
    fn fetch(&self) -> Result<FetchedToken, AccessTokenError>;
}

pub trait Clock: Send + Sync {
    fn now_epoch_seconds(&self) -> u64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_epoch_seconds(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs())
    }
}
