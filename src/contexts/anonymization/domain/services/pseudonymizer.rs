use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use unicode_normalization::UnicodeNormalization;

use super::generators;
use super::keys::Keys;
use crate::contexts::anonymization::domain::repositories::PseudonymStore;
use crate::contexts::anonymization::domain::types::{
    GeneratorConfig, Kind, Pseudonym, StoreOutcome,
};

const MAX_DRAWS: usize = 64;

pub struct Pseudonymizer {
    keys: Keys,
    store: Box<dyn PseudonymStore>,
    config: GeneratorConfig,
}

impl Pseudonymizer {
    pub fn new(keys: Keys, store: Box<dyn PseudonymStore>, config: GeneratorConfig) -> Self {
        Self {
            keys,
            store,
            config,
        }
    }

    pub fn pseudonymize(&self, kind: Kind, value: &str) -> Pseudonym {
        if value.trim().is_empty() {
            return Pseudonym {
                text: value.to_string(),
                reversible: false,
                exhausted: false,
            };
        }
        let normalized: String = value.nfc().collect();
        let seed = self.keys.seed(kind, &normalized);
        let mut rng = ChaCha20Rng::from_seed(seed);
        if !kind.is_reversible() {
            return Pseudonym {
                text: generators::generate(kind, &normalized, &mut rng, &self.config),
                reversible: false,
                exhausted: false,
            };
        }
        let outcome = self.store.get_or_create(
            kind,
            &seed,
            MAX_DRAWS,
            &mut || generators::generate(kind, &normalized, &mut rng, &self.config),
            &|| self.keys.encrypt(kind, &normalized),
        );
        match outcome {
            Ok(StoreOutcome::Known(text) | StoreOutcome::Created(text)) => Pseudonym {
                text,
                reversible: true,
                exhausted: false,
            },
            Ok(StoreOutcome::Exhausted) => {
                tracing::warn!(
                    kind = kind.as_str(),
                    "pseudonym namespace exhausted; an opaque token replaces the value"
                );
                opaque(kind, &seed)
            }
            Err(error) => {
                tracing::warn!(
                    kind = kind.as_str(),
                    %error,
                    "pseudonym store unavailable; an opaque token replaces the value"
                );
                opaque(kind, &seed)
            }
        }
    }

    pub fn reverse(&self, text: &str) -> Option<(Kind, String)> {
        let rows = match self.store.find(text) {
            Ok(rows) => rows,
            Err(error) => {
                tracing::warn!(%error, "pseudonym store lookup failed");
                return None;
            }
        };
        rows.into_iter().find_map(|(name, ciphertext)| {
            let kind = Kind::parse(&name).filter(|kind| kind.is_reversible())?;
            self.keys
                .decrypt(kind, &ciphertext)
                .map(|value| (kind, value))
        })
    }

    pub fn looks_like_pseudonym(&self, text: &str) -> Option<Kind> {
        generators::recognise(text)
    }

    pub fn opaque_token(&self, label: &str, value: &str) -> String {
        let seed = self
            .keys
            .seed(Kind::PersonName, &format!("opaque\u{0}{label}\u{0}{value}"));
        format!("<{label} id={}>", hex::encode(&seed[..4]))
    }
}

fn opaque(kind: Kind, seed: &[u8; 32]) -> Pseudonym {
    Pseudonym {
        text: format!("<{} id={}>", kind.as_str(), hex::encode(&seed[..4])),
        reversible: false,
        exhausted: true,
    }
}
