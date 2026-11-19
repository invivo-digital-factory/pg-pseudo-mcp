use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::time::{Duration, Instant};

use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use hmac::{Hmac, Mac};
use rand::TryRng;
use rand::rngs::SysRng;
use sha2::Sha256;

use super::PseudoError;
use crate::kind::Kind;

pub(crate) const SECRET_LEN: usize = 32;
const NONCE_LEN: usize = 24;
const PRIVATE_FILE_MODE: u32 = 0o600;
const PARTIAL_SECRET_WAIT: Duration = Duration::from_secs(2);

pub(crate) struct Keys {
    secret: [u8; SECRET_LEN],
    cipher: XChaCha20Poly1305,
}

impl Keys {
    pub(crate) fn new(secret: [u8; SECRET_LEN]) -> Self {
        let store_key = hmac(&secret, &[b"store-key"]);
        Self {
            secret,
            cipher: XChaCha20Poly1305::new(&store_key.into()),
        }
    }

    pub(crate) fn seed(&self, kind: Kind, normalized_value: &str) -> [u8; 32] {
        hmac(
            &self.secret,
            &[kind.as_str().as_bytes(), &[0], normalized_value.as_bytes()],
        )
    }

    pub(crate) fn key_check(&self) -> [u8; 32] {
        hmac(&self.secret, &[b"key-check"])
    }

    pub(crate) fn encrypt(&self, kind: Kind, plaintext: &str) -> Result<Vec<u8>, PseudoError> {
        let nonce: [u8; NONCE_LEN] = random_bytes()?;
        let ciphertext = self
            .cipher
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: plaintext.as_bytes(),
                    aad: kind.as_str().as_bytes(),
                },
            )
            .map_err(|_| PseudoError::Crypto)?;
        let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(&nonce);
        blob.extend_from_slice(&ciphertext);
        Ok(blob)
    }

    pub(crate) fn decrypt(&self, kind: Kind, blob: &[u8]) -> Option<String> {
        if blob.len() < NONCE_LEN {
            return None;
        }
        let (nonce, ciphertext) = blob.split_at(NONCE_LEN);
        let nonce: [u8; NONCE_LEN] = nonce.try_into().ok()?;
        let plaintext = self
            .cipher
            .decrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: ciphertext,
                    aad: kind.as_str().as_bytes(),
                },
            )
            .ok()?;
        String::from_utf8(plaintext).ok()
    }
}

fn hmac(key: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key)
        .unwrap_or_else(|_| unreachable!("HMAC accepts keys of any length"));
    for part in parts {
        mac.update(part);
    }
    mac.finalize().into_bytes().into()
}

pub(crate) fn random_bytes<const N: usize>() -> Result<[u8; N], PseudoError> {
    let mut bytes = [0u8; N];
    SysRng
        .try_fill_bytes(&mut bytes)
        .map_err(|error| PseudoError::Random(error.to_string()))?;
    Ok(bytes)
}

pub(crate) fn load_or_create(path: &Path) -> Result<[u8; SECRET_LEN], PseudoError> {
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(PRIVATE_FILE_MODE)
        .open(path)
    {
        Ok(mut file) => {
            let written = random_bytes::<SECRET_LEN>().and_then(|secret| {
                file.write_all(&secret)
                    .and_then(|()| file.sync_all())
                    .map_err(|source| PseudoError::io("write", path, source))?;
                Ok(secret)
            });
            if written.is_err() {
                let _ = fs::remove_file(path);
            }
            written
        }
        Err(error) if error.kind() == ErrorKind::AlreadyExists => read_existing(path),
        Err(source) => Err(PseudoError::io("create", path, source)),
    }
}

fn read_existing(path: &Path) -> Result<[u8; SECRET_LEN], PseudoError> {
    let started = Instant::now();
    loop {
        let mut bytes = Vec::with_capacity(SECRET_LEN + 1);
        fs::File::open(path)
            .and_then(|mut file| file.read_to_end(&mut bytes))
            .map_err(|source| PseudoError::io("read", path, source))?;
        if let Ok(secret) = <[u8; SECRET_LEN]>::try_from(bytes.as_slice()) {
            restrict_permissions(path)?;
            return Ok(secret);
        }
        if bytes.len() > SECRET_LEN || started.elapsed() > PARTIAL_SECRET_WAIT {
            return Err(PseudoError::CorruptSecret {
                path: path.to_path_buf(),
            });
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub(crate) fn restrict_permissions(path: &Path) -> Result<(), PseudoError> {
    let metadata = fs::metadata(path).map_err(|source| PseudoError::io("inspect", path, source))?;
    if metadata.permissions().mode() & 0o777 != PRIVATE_FILE_MODE {
        fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_FILE_MODE))
            .map_err(|source| PseudoError::io("restrict", path, source))?;
    }
    Ok(())
}

pub(crate) fn create_private_file(path: &Path) -> Result<(), PseudoError> {
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(PRIVATE_FILE_MODE)
        .open(path)
        .map_err(|source| PseudoError::io("create", path, source))?;
    restrict_permissions(path)
}
