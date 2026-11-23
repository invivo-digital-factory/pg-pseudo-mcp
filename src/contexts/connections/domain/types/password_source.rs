use super::connection_profile::SslMode;
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

    pub fn effective_sslmode(&self, requested: SslMode) -> SslMode {
        if self.is_azure_token() {
            requested.at_least_require()
        } else {
            requested
        }
    }

    pub fn command(&self) -> Option<&[String]> {
        match self {
            Self::Command(argv) => Some(argv),
            _ => None,
        }
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn un_jeton_azure_releve_le_mode_ssl_au_moins_a_require_et_les_autres_sources_le_laissent() {
        let token = PasswordSource::AzureToken(Secret::new("t"));
        assert_eq!(token.effective_sslmode(SslMode::Disable), SslMode::Require);
        assert_eq!(token.effective_sslmode(SslMode::Prefer), SslMode::Require);
        assert_eq!(
            token.effective_sslmode(SslMode::VerifyFull),
            SslMode::VerifyFull
        );
        for source in [
            PasswordSource::Resolved(Secret::new("p")),
            PasswordSource::Command(vec!["cmd".to_string()]),
        ] {
            assert_eq!(source.effective_sslmode(SslMode::Disable), SslMode::Disable);
        }
    }
}
