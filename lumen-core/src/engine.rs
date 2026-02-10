//! Storage engine: coordinates the in-memory BTreeMap (memtable) and the WAL.
//!
//! Write path:  WAL append  →  memtable insert  (durable before visible)
//! Read path:   memtable only  (no SSTables in this iteration)

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use thiserror::Error;
use tracing::{debug, info};

use crate::wal::{WalError, WalRecord, WriteAheadLog};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("WAL error: {0}")]
    Wal(#[from] WalError),

    #[error("Internal lock was poisoned; the process may be in an inconsistent state")]
    LockPoisoned,
}

/// Map any `PoisonError` variant into `EngineError::LockPoisoned`.
impl<T> From<PoisonError<T>> for EngineError {
    fn from(_: PoisonError<T>) -> Self {
        EngineError::LockPoisoned
    }
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Thread-safe LSM-inspired key-value engine backed by a WAL.
///
/// Cloning an `Engine` is cheap — both clones share the same storage state.
#[derive(Clone, Debug)]
pub struct Engine {
    /// In-memory sorted map of live key→value pairs.
    memtable: Arc<RwLock<BTreeMap<String, Vec<u8>>>>,
    /// Serialised access to the WAL writer (one writer at a time).
    wal: Arc<Mutex<WriteAheadLog>>,
    _data_dir: Arc<PathBuf>,
}

impl Engine {
    /// Open the engine rooted at `data_dir`.
    ///
    /// 1. Creates the directory if absent.
    /// 2. Replays the WAL to rebuild the memtable.
    /// 3. Opens the WAL in append mode, ready for new writes.
    pub fn open(data_dir: impl Into<PathBuf>) -> Result<Self, EngineError> {
        let data_dir = data_dir.into();

        std::fs::create_dir_all(&data_dir).map_err(WalError::Io)?;

        let wal_path = data_dir.join("wal.log");

        // ── Replay WAL ──────────────────────────────────────────────────────
        let records  = WriteAheadLog::recover(&wal_path)?;
        let mut map  = BTreeMap::new();

        for record in &records {
            match record {
                WalRecord::Put { key, value } => { map.insert(key.clone(), value.clone()); }
                WalRecord::Delete { key }     => { map.remove(key); }
            }
        }

        info!(
            data_dir  = %data_dir.display(),
            recovered = map.len(),
            wal_ops   = records.len(),
            "Engine initialised"
        );

        // ── Open WAL for appending ──────────────────────────────────────────
        let wal = WriteAheadLog::open(&wal_path)?;

        Ok(Self {
            memtable:  Arc::new(RwLock::new(map)),
            wal:       Arc::new(Mutex::new(wal)),
            _data_dir: Arc::new(data_dir),
        })
    }

    // ── Write operations ────────────────────────────────────────────────────

    /// Insert or overwrite `key` with `value`.
    ///
    /// The WAL entry is flushed before the memtable is updated so that a crash
    /// between the two steps is recoverable on restart.
    pub fn put(&self, key: String, value: Vec<u8>) -> Result<(), EngineError> {
        debug!(key = %key, bytes = value.len(), "PUT");

        {
            let mut wal = self.wal.lock()?;
            wal.append(&WalRecord::Put { key: key.clone(), value: value.clone() })?;
        }

        let mut mem = self.memtable.write()?;
        mem.insert(key, value);

        Ok(())
    }

    /// Remove `key` from the store.  
    /// Returns `true` if the key existed, `false` otherwise.
    pub fn delete(&self, key: &str) -> Result<bool, EngineError> {
        debug!(key = %key, "DELETE");

        {
            let mut wal = self.wal.lock()?;
            wal.append(&WalRecord::Delete { key: key.to_owned() })?;
        }

        let mut mem = self.memtable.write()?;
        Ok(mem.remove(key).is_some())
    }

    // ── Read operations ─────────────────────────────────────────────────────

    /// Look up `key`.  Returns `None` if the key does not exist.
    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>, EngineError> {
        debug!(key = %key, "GET");
        let mem = self.memtable.read()?;
        Ok(mem.get(key).cloned())
    }

    // ── Diagnostics ─────────────────────────────────────────────────────────

    /// Number of live keys currently held in memory.
    pub fn len(&self) -> Result<usize, EngineError> {
        Ok(self.memtable.read()?.len())
    }

    /// Returns `true` if the store contains no keys.
    pub fn is_empty(&self) -> Result<bool, EngineError> {
        Ok(self.len()? == 0)
    }
}
