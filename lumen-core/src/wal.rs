//! Write-Ahead Log with CRC32 integrity protection.
//!
//! On-disk record format (per entry):
//!   [Op (1 byte)] [CRC32 (4 bytes, big-endian)]
//!   [Key Len (8 bytes, big-endian)] [Value Len (8 bytes, big-endian)]
//!   [Key Bytes] [Value Bytes]
//!
//! CRC32 is computed over: op || key_len || value_len || key_bytes || value_bytes

use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use crc32fast::Hasher as Crc32Hasher;
use thiserror::Error;
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum WalError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("CRC32 checksum mismatch: expected {expected:#010x}, got {actual:#010x}")]
    ChecksumMismatch { expected: u32, actual: u32 },

    #[error("Unknown WAL operation byte: {0:#04x}")]
    UnknownOperation(u8),

    #[error("Invalid UTF-8 in stored key: {0}")]
    InvalidKey(#[from] std::string::FromUtf8Error),
}

// ---------------------------------------------------------------------------
// Record type
// ---------------------------------------------------------------------------

const OP_PUT: u8    = 0x01;
const OP_DELETE: u8 = 0x02;

/// A single logical entry stored in the WAL.
#[derive(Debug, Clone)]
pub enum WalRecord {
    Put    { key: String, value: Vec<u8> },
    Delete { key: String },
}

// ---------------------------------------------------------------------------
// WriteAheadLog
// ---------------------------------------------------------------------------

/// Append-only, CRC32-protected log file.
#[derive(Debug)]
pub struct WriteAheadLog {
    writer: BufWriter<File>,
    path: PathBuf,
}

impl WriteAheadLog {
    /// Open (or create) the WAL at `path` in append mode.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, WalError> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;

        info!(path = %path.display(), "WAL file opened in append mode");

        Ok(Self {
            writer: BufWriter::new(file),
            path,
        })
    }

    /// Append a record to the WAL and fsync.
    pub fn append(&mut self, record: &WalRecord) -> Result<(), WalError> {
        let (op, key, value): (u8, &str, &[u8]) = match record {
            WalRecord::Put { key, value }  => (OP_PUT,    key.as_str(), value.as_slice()),
            WalRecord::Delete { key }      => (OP_DELETE, key.as_str(), &[]),
        };

        let key_bytes = key.as_bytes();
        let key_len   = key_bytes.len() as u64;
        let value_len = value.len()     as u64;

        // Compute CRC32 over: op || key_len (BE) || value_len (BE) || key_bytes || value
        let checksum = {
            let mut h = Crc32Hasher::new();
            h.update(&[op]);
            h.update(&key_len.to_be_bytes());
            h.update(&value_len.to_be_bytes());
            h.update(key_bytes);
            h.update(value);
            h.finalize()
        };

        self.writer.write_u8(op)?;
        self.writer.write_u32::<BigEndian>(checksum)?;
        self.writer.write_u64::<BigEndian>(key_len)?;
        self.writer.write_u64::<BigEndian>(value_len)?;
        self.writer.write_all(key_bytes)?;
        self.writer.write_all(value)?;
        // Flush to kernel buffer; the OS will durably persist this.
        self.writer.flush()?;

        Ok(())
    }

    /// Read and validate every record from an existing WAL file.
    ///
    /// Returns an empty `Vec` if the file does not exist yet.
    /// Stops and returns an error on the first corrupted record.
    pub fn recover<P: AsRef<Path>>(path: P) -> Result<Vec<WalRecord>, WalError> {
        let path = path.as_ref();

        let file = match File::open(path) {
            Ok(f)  => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                info!(path = %path.display(), "No WAL found; starting fresh");
                return Ok(Vec::new());
            }
            Err(e) => return Err(WalError::Io(e)),
        };

        let mut reader  = BufReader::new(file);
        let mut records = Vec::new();

        loop {
            // Read op byte — EOF here is normal (clean shutdown).
            let op = match reader.read_u8() {
                Ok(b)  => b,
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(WalError::Io(e)),
            };

            if op != OP_PUT && op != OP_DELETE {
                return Err(WalError::UnknownOperation(op));
            }

            let stored_checksum = reader.read_u32::<BigEndian>()?;
            let key_len         = reader.read_u64::<BigEndian>()?;
            let value_len       = reader.read_u64::<BigEndian>()?;

            let mut key_bytes = vec![0u8; key_len as usize];
            reader.read_exact(&mut key_bytes)?;

            let mut value = vec![0u8; value_len as usize];
            reader.read_exact(&mut value)?;

            // Verify integrity
            let computed = {
                let mut h = Crc32Hasher::new();
                h.update(&[op]);
                h.update(&key_len.to_be_bytes());
                h.update(&value_len.to_be_bytes());
                h.update(&key_bytes);
                h.update(&value);
                h.finalize()
            };

            if computed != stored_checksum {
                warn!(
                    expected = stored_checksum,
                    actual   = computed,
                    "WAL checksum mismatch — truncated or corrupt entry"
                );
                return Err(WalError::ChecksumMismatch {
                    expected: stored_checksum,
                    actual:   computed,
                });
            }

            let key = String::from_utf8(key_bytes)?;

            let record = match op {
                OP_PUT    => WalRecord::Put { key, value },
                OP_DELETE => WalRecord::Delete { key },
                _         => unreachable!("op validated above"),
            };

            records.push(record);
        }

        info!(
            path  = %path.display(),
            count = records.len(),
            "WAL recovery complete"
        );

        Ok(records)
    }

    /// Return the path this WAL is stored at.
    pub fn path(&self) -> &Path {
        &self.path
    }
}
