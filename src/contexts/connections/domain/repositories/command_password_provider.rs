use crate::contexts::connections::domain::errors::PasswordCommandError;
use crate::contexts::connections::domain::types::Secret;

pub trait CommandPasswordProvider: Send + Sync {
    fn password(&self, connection: &str, argv: &[String]) -> Result<Secret, PasswordCommandError>;

    fn renewed_password(
        &self,
        connection: &str,
        argv: &[String],
    ) -> Result<Secret, PasswordCommandError>;
}
