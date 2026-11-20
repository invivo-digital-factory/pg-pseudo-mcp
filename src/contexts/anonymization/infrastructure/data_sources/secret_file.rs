use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::time::{Duration, Instant};

use crate::contexts::anonymization::domain::errors::PseudoError;
use crate::contexts::anonymization::domain::services::keys::{SECRET_LEN, random_bytes};

const PRIVATE_FILE_MODE: u32 = 0o600;
const PARTIAL_SECRET_WAIT: Duration = Duration::from_secs(2);

pub fn load_or_create(path: &Path) -> Result<[u8; SECRET_LEN], PseudoError> {
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

pub fn restrict_permissions(path: &Path) -> Result<(), PseudoError> {
    let metadata = fs::metadata(path).map_err(|source| PseudoError::io("inspect", path, source))?;
    if metadata.permissions().mode() & 0o777 != PRIVATE_FILE_MODE {
        fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_FILE_MODE))
            .map_err(|source| PseudoError::io("restrict", path, source))?;
    }
    Ok(())
}

pub fn create_private_file(path: &Path) -> Result<(), PseudoError> {
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(PRIVATE_FILE_MODE)
        .open(path)
        .map_err(|source| PseudoError::io("create", path, source))?;
    restrict_permissions(path)
}
