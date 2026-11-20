use crate::contexts::connections::domain::errors::AccessTokenError;
use crate::contexts::connections::domain::types::Secret;

pub trait AzureSignInTokenProvider: Send + Sync {
    fn access_token(&self) -> Result<Secret, AccessTokenError>;
    fn renewed_access_token(&self) -> Result<Secret, AccessTokenError>;
}
