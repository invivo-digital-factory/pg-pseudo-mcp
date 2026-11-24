use std::fmt;
use std::io;

use rand::TryRng;
use rand::rngs::SysRng;
use subtle::ConstantTimeEq;

const SECRET_LEN: usize = 32;

#[derive(Clone)]
pub struct ProxyCredentials {
    username: String,
    password: String,
}

impl ProxyCredentials {
    pub fn generate() -> io::Result<Self> {
        Ok(Self {
            username: random_secret()?,
            password: random_secret()?,
        })
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn password(&self) -> &str {
        &self.password
    }

    pub(super) fn matches(&self, username: &[u8], password: &[u8]) -> bool {
        let same_username = self.username.as_bytes().ct_eq(username);
        let same_password = self.password.as_bytes().ct_eq(password);
        (same_username & same_password).into()
    }
}

impl fmt::Debug for ProxyCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProxyCredentials(<redacted>)")
    }
}

fn random_secret() -> io::Result<String> {
    let mut bytes = [0u8; SECRET_LEN];
    SysRng
        .try_fill_bytes(&mut bytes)
        .map_err(io::Error::other)?;
    Ok(hex::encode(bytes))
}
