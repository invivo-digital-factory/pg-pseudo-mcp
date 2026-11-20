#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AccessTokenError {
    #[error("the Azure CLI (az) was not found on PATH; install it, then run az login")]
    CliNotInstalled,
    #[error("no Azure access token available, run az login ({detail})")]
    SignInRequired { detail: String },
    #[error("the Azure CLI answered with an access token that cannot be read; run az login again")]
    Unreadable,
}
