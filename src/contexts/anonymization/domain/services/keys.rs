use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use hmac::{Hmac, Mac};
use rand::TryRng;
use rand::rngs::SysRng;
use sha2::Sha256;

use crate::contexts::anonymization::domain::errors::PseudoError;
use crate::contexts::anonymization::domain::types::Kind;

pub const SECRET_LEN: usize = 32;
const NONCE_LEN: usize = 24;

pub struct Keys {
    secret: [u8; SECRET_LEN],
    cipher: XChaCha20Poly1305,
}

impl Keys {
    pub fn new(secret: [u8; SECRET_LEN]) -> Self {
        let store_key = hmac(&secret, &[b"store-key"]);
        Self {
            secret,
            cipher: XChaCha20Poly1305::new(&store_key.into()),
        }
    }

    pub fn seed(&self, kind: Kind, normalized_value: &str) -> [u8; 32] {
        hmac(
            &self.secret,
            &[kind.as_str().as_bytes(), &[0], normalized_value.as_bytes()],
        )
    }

    pub fn key_check(&self) -> [u8; 32] {
        hmac(&self.secret, &[b"key-check"])
    }

    pub fn encrypt(&self, kind: Kind, plaintext: &str) -> Result<Vec<u8>, PseudoError> {
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

    pub fn decrypt(&self, kind: Kind, blob: &[u8]) -> Option<String> {
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

pub fn random_bytes<const N: usize>() -> Result<[u8; N], PseudoError> {
    let mut bytes = [0u8; N];
    SysRng
        .try_fill_bytes(&mut bytes)
        .map_err(|error| PseudoError::Random(error.to_string()))?;
    Ok(bytes)
}
