use std::collections::HashMap;
use std::fmt;

use aes::Aes128;
use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
use serde_json::Value;

type Aes128CbcDec = cbc::Decryptor<Aes128>;
type Aes128CbcEnc = cbc::Encryptor<Aes128>;

const KEY: [u8; 16] = [
    0xba, 0xbb, 0x4a, 0x9f, 0x77, 0x4a, 0xb8, 0x53, 0xc9, 0x6c, 0x2d, 0x65, 0x3d, 0xfe, 0x54, 0x4a,
];
const IV_LEN: usize = 16;

#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

#[derive(Debug, Clone, Default)]
pub struct SavedCredentials {
    pub user: Option<String>,
    pub password: Option<Secret>,
}

#[derive(Debug, Default)]
pub struct CredentialStore {
    entries: HashMap<String, SavedCredentials>,
    ssh_entries: HashMap<String, SavedCredentials>,
}

impl CredentialStore {
    pub fn get(&self, connection_id: &str) -> Option<&SavedCredentials> {
        self.entries.get(connection_id)
    }

    pub fn ssh(&self, connection_id: &str) -> Option<&SavedCredentials> {
        self.ssh_entries.get(connection_id)
    }

    pub fn from_encrypted(bytes: &[u8]) -> Result<Self, CredentialsError> {
        let plaintext = decrypt(bytes)?;
        let root: Value =
            serde_json::from_slice(&plaintext).map_err(|_| CredentialsError::Malformed)?;
        let object = root.as_object().ok_or(CredentialsError::Malformed)?;
        let mut entries = HashMap::new();
        let mut ssh_entries = HashMap::new();
        for (id, value) in object {
            if let Some(saved) = saved_at(value, "#connection") {
                entries.insert(id.clone(), saved);
            }
            if let Some(saved) = saved_at(value, "network/ssh_tunnel") {
                ssh_entries.insert(id.clone(), saved);
            }
        }
        Ok(Self {
            entries,
            ssh_entries,
        })
    }
}

fn saved_at(value: &Value, node: &str) -> Option<SavedCredentials> {
    let node = value.get(node)?.as_object()?;
    let text = |key: &str| node.get(key).and_then(Value::as_str).map(str::to_string);
    Some(SavedCredentials {
        user: text("user"),
        password: text("password").map(Secret::new),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CredentialsError {
    #[error("credentials file is too short")]
    TooShort,
    #[error("credentials file could not be decrypted")]
    Decrypt,
    #[error("credentials file is not valid JSON")]
    Malformed,
}

fn decrypt(bytes: &[u8]) -> Result<Vec<u8>, CredentialsError> {
    if bytes.len() <= IV_LEN {
        return Err(CredentialsError::TooShort);
    }
    let (iv, ciphertext) = bytes.split_at(IV_LEN);
    let iv: [u8; IV_LEN] = iv.try_into().map_err(|_| CredentialsError::TooShort)?;
    let mut buffer = ciphertext.to_vec();
    let plaintext = Aes128CbcDec::new(&KEY.into(), &iv.into())
        .decrypt_padded::<Pkcs7>(&mut buffer)
        .map_err(|_| CredentialsError::Decrypt)?;
    Ok(plaintext.to_vec())
}

pub fn encrypt_for_tests(json: &str, iv: [u8; IV_LEN]) -> Vec<u8> {
    let message = json.as_bytes();
    let mut buffer = vec![0u8; message.len() + IV_LEN];
    buffer[..message.len()].copy_from_slice(message);
    let ciphertext = Aes128CbcEnc::new(&KEY.into(), &iv.into())
        .encrypt_padded::<Pkcs7>(&mut buffer, message.len())
        .expect("buffer holds one spare block for padding");
    let mut output = iv.to_vec();
    output.extend_from_slice(ciphertext);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_connection_credentials() {
        let json = r##"{"c1":{"#connection":{"user":"u","password":"p"},"network/ssh_tunnel":{"user":"x"}},"c2":{}}"##;
        let bytes = encrypt_for_tests(json, [7; 16]);
        let store = CredentialStore::from_encrypted(&bytes).unwrap();
        let saved = store.get("c1").unwrap();
        assert_eq!(saved.user.as_deref(), Some("u"));
        assert_eq!(saved.password.as_ref().unwrap().expose(), "p");
        assert!(store.get("c2").is_none());
    }

    #[test]
    fn reads_ssh_tunnel_credentials_even_without_a_connection_node() {
        let json = r##"{"c1":{"#connection":{"user":"u"},"network/ssh_tunnel":{"user":"ssh-u","password":"ssh-p"}},"c2":{"network/ssh_tunnel":{"user":"only-ssh"}}}"##;
        let store = CredentialStore::from_encrypted(&encrypt_for_tests(json, [3; 16])).unwrap();
        let ssh = store.ssh("c1").unwrap();
        assert_eq!(ssh.user.as_deref(), Some("ssh-u"));
        assert_eq!(ssh.password.as_ref().unwrap().expose(), "ssh-p");
        assert_eq!(store.ssh("c2").unwrap().user.as_deref(), Some("only-ssh"));
        assert!(store.ssh("c2").unwrap().password.is_none());
        assert!(store.get("c2").is_none());
        assert!(store.ssh("c3").is_none());
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(CredentialStore::from_encrypted(&[1; 8]).is_err());
        assert!(CredentialStore::from_encrypted(&[1; 48]).is_err());
    }

    #[test]
    fn secret_never_prints() {
        let secret = Secret::new("hunter2");
        assert_eq!(format!("{secret:?}{secret}"), "******");
    }
}
