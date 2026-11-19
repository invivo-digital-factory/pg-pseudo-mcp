use std::collections::HashMap;

use sha2::{Digest, Sha256};

use crate::contexts::connections::domain::types::ConnectionEntry;

pub fn disambiguate(connections: &mut [ConnectionEntry]) {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for entry in connections.iter() {
        *counts.entry(entry.name.clone()).or_default() += 1;
    }
    for entry in connections.iter_mut() {
        if counts.get(&entry.name).copied().unwrap_or(0) > 1 {
            let digest = Sha256::digest(format!("{}/{}", entry.project, entry.id).as_bytes());
            let suffix = hex::encode(&digest[..2]);
            entry.name = format!("{} ({suffix})", entry.name);
        }
    }
}
