use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SslMode {
    Disable,
    Allow,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ConnectOptions {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: String,
    pub sslmode: SslMode,
}

impl fmt::Debug for ConnectOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectOptions")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("database", &self.database)
            .field("user", &self.user)
            .field("password", &"<redacted>")
            .field("sslmode", &self.sslmode)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_never_contains_the_password() {
        let options = ConnectOptions {
            host: "localhost".into(),
            port: 5432,
            database: "app".into(),
            user: "reader".into(),
            password: "s3cr3t-value".into(),
            sslmode: SslMode::Prefer,
        };
        let printed = format!("{options:?}");
        assert!(!printed.contains("s3cr3t-value"));
        assert!(printed.contains("reader"));
    }
}
