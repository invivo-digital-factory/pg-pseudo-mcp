use super::secret::Secret;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasswordSource {
    Resolved(Secret),
    AzureToken(Secret),
    Command(Vec<String>),
}

impl PasswordSource {
    pub fn expose(&self) -> &str {
        match self {
            Self::Resolved(secret) | Self::AzureToken(secret) => secret.expose(),
            Self::Command(_) => "",
        }
    }

    pub fn is_azure_token(&self) -> bool {
        matches!(self, Self::AzureToken(_))
    }

    pub fn command(&self) -> Option<&[String]> {
        match self {
            Self::Command(argv) => Some(argv),
            _ => None,
        }
    }
}
