#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PasswordCommandError {
    #[error("the password command was not found")]
    NotFound,
    #[error("the password command could not be started")]
    CouldNotStart,
    #[error("the password command exited with status {0}")]
    ExitStatus(i32),
    #[error("the password command was stopped by a signal")]
    Interrupted,
    #[error("the password command did not finish in time")]
    TimedOut,
    #[error("the password command printed more than the allowed output")]
    OutputTooLarge,
    #[error("the password command printed nothing")]
    NoOutput,
    #[error("the password command output is not valid UTF-8")]
    NotUtf8,
}
