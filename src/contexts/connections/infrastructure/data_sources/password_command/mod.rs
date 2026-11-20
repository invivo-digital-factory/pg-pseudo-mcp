mod runner;

pub use runner::ProcessPasswordCommand;

use crate::contexts::connections::domain::errors::PasswordCommandError;
use crate::contexts::connections::domain::types::Secret;

pub trait PasswordCommandRunner: Send + Sync {
    fn run(&self, argv: &[String]) -> Result<Secret, PasswordCommandError>;
}
