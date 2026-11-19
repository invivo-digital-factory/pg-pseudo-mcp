use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use rusqlite::{Connection, ErrorCode, OptionalExtension, TransactionBehavior, params};

use crate::contexts::anonymization::domain::errors::PseudoError;
use crate::contexts::anonymization::domain::repositories::PseudonymStore;
use crate::contexts::anonymization::domain::types::{Kind, StoreOutcome};
use crate::contexts::anonymization::infrastructure::data_sources::secret_file::{
    create_private_file, restrict_permissions,
};

const BUSY_TIMEOUT: Duration = Duration::from_secs(10);
const BUSY_RETRY_PAUSE: Duration = Duration::from_millis(10);
const KEY_CHECK: &str = "key_check";
const CURRENT_KEY_ID: i64 = 1;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS mappings (
    kind TEXT NOT NULL,
    pseudonym TEXT NOT NULL,
    forward_key BLOB NOT NULL,
    real_ct BLOB NOT NULL,
    key_id INTEGER NOT NULL,
    UNIQUE (kind, pseudonym),
    UNIQUE (kind, forward_key)
);
CREATE INDEX IF NOT EXISTS mappings_by_pseudonym ON mappings (pseudonym);
CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value BLOB NOT NULL
);
";

pub struct SqlitePseudonymStore {
    connection: Mutex<Connection>,
}

impl SqlitePseudonymStore {
    pub fn open(path: &Path, key_check: &[u8; 32]) -> Result<Self, PseudoError> {
        create_private_file(path)?;
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(BUSY_TIMEOUT)?;
        let started = Instant::now();
        let mode: String = retry_busy(started, || {
            connection.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
        })?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(PseudoError::WalUnavailable {
                store: path.to_path_buf(),
                mode,
            });
        }
        retry_busy(started, || {
            connection.pragma_update(None, "synchronous", "NORMAL")
        })?;
        retry_busy(started, || create_schema(&mut connection))?;
        verify_key(&connection, started, path, key_check)?;
        for companion in companion_files(path) {
            if companion.exists() {
                restrict_permissions(&companion)?;
            }
        }
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }
}

impl PseudonymStore for SqlitePseudonymStore {
    fn get_or_create(
        &self,
        kind: Kind,
        forward_key: &[u8; 32],
        max_draws: usize,
        draw: &mut dyn FnMut() -> String,
        encrypt: &dyn Fn() -> Result<Vec<u8>, PseudoError>,
    ) -> Result<StoreOutcome, PseudoError> {
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let known: Option<String> = transaction
            .query_row(
                "SELECT pseudonym FROM mappings WHERE kind = ?1 AND forward_key = ?2",
                params![kind.as_str(), forward_key.as_slice()],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(pseudonym) = known {
            return Ok(StoreOutcome::Known(pseudonym));
        }
        let mut taken = transaction
            .prepare_cached("SELECT 1 FROM mappings WHERE kind = ?1 AND pseudonym = ?2")?;
        let mut chosen = None;
        for _ in 0..max_draws {
            let candidate = draw();
            if !taken.exists(params![kind.as_str(), candidate])? {
                chosen = Some(candidate);
                break;
            }
        }
        drop(taken);
        let Some(pseudonym) = chosen else {
            return Ok(StoreOutcome::Exhausted);
        };
        transaction.execute(
            "INSERT INTO mappings (kind, pseudonym, forward_key, real_ct, key_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                kind.as_str(),
                pseudonym,
                forward_key.as_slice(),
                encrypt()?,
                CURRENT_KEY_ID
            ],
        )?;
        transaction.commit()?;
        Ok(StoreOutcome::Created(pseudonym))
    }

    fn find(&self, pseudonym: &str) -> Result<Vec<(String, Vec<u8>)>, PseudoError> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let mut statement = connection.prepare_cached(
            "SELECT kind, real_ct FROM mappings WHERE pseudonym = ?1 ORDER BY kind",
        )?;
        let rows = statement
            .query_map(params![pseudonym], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

fn create_schema(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute_batch(SCHEMA)?;
    transaction.commit()
}

fn verify_key(
    connection: &Connection,
    started: Instant,
    path: &Path,
    key_check: &[u8; 32],
) -> Result<(), PseudoError> {
    let stored: Vec<u8> = retry_busy(started, || {
        connection.execute(
            "INSERT OR IGNORE INTO meta (key, value) VALUES (?1, ?2)",
            params![KEY_CHECK, key_check.as_slice()],
        )?;
        connection.query_row(
            "SELECT value FROM meta WHERE key = ?1",
            params![KEY_CHECK],
            |row| row.get(0),
        )
    })?;
    if stored.as_slice() != key_check.as_slice() {
        return Err(PseudoError::SecretMismatch {
            store: path.to_path_buf(),
        });
    }
    Ok(())
}

fn retry_busy<T>(
    started: Instant,
    mut step: impl FnMut() -> rusqlite::Result<T>,
) -> rusqlite::Result<T> {
    loop {
        match step() {
            Err(error) if is_busy(&error) && started.elapsed() < BUSY_TIMEOUT => {
                std::thread::sleep(BUSY_RETRY_PAUSE);
            }
            outcome => return outcome,
        }
    }
}

fn is_busy(error: &rusqlite::Error) -> bool {
    matches!(
        error.sqlite_error_code(),
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
    )
}

pub fn companion_files(path: &Path) -> [PathBuf; 2] {
    ["-wal", "-shm"].map(|suffix| {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        PathBuf::from(name)
    })
}
