use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Read, Seek, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::book::OrderBook;
use crate::types::Order;

/// A command recorded in the WAL — sufficient to deterministically replay book state.
#[allow(clippy::module_name_repetitions)]
///
/// PRD §14.6: commands (not derived events) are the right WAL payload because the matching
/// engine is deterministic: re-running the same command sequence always produces the same book.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalCommand {
    /// An order was submitted to the engine.
    PlaceOrder(Order),
    /// An order was cancelled by ID.
    CancelOrder(Uuid),
}

/// A single WAL record with a monotonic sequence number.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEntry {
    /// Monotonic sequence; starts at 1, increments by 1 per append.
    pub seq: u64,
    /// The command to replay.
    pub command: WalCommand,
}

/// Append-only write-ahead log using length-prefixed bincode encoding.
///
/// Wire format per PRD §14.6: `[u32 LE length][bincode(WalEntry)]`.
pub struct Wal {
    path: PathBuf,
    file: BufWriter<File>,
    /// Sequence number of the last successfully written entry.
    pub sequence: u64,
}

impl Wal {
    /// Opens (or creates) a WAL file, scanning existing entries to restore `sequence`.
    ///
    /// # Errors
    ///
    /// Returns an IO error if the file cannot be opened or if the existing entries are
    /// unreadable beyond what can be recovered by tail truncation.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let existing_seq = Self::find_last_seq(&path)?;
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self {
            path,
            file: BufWriter::new(file),
            sequence: existing_seq,
        })
    }

    /// Appends a command to the WAL, flushing before returning.
    ///
    /// # Errors
    ///
    /// Returns an IO error if the write or flush fails.
    pub fn append(&mut self, command: WalCommand) -> io::Result<()> {
        self.sequence += 1;
        let entry = WalEntry {
            seq: self.sequence,
            command,
        };
        let bytes = bincode::serialize(&entry)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let len = u32::try_from(bytes.len())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        self.file.write_all(&len.to_le_bytes())?;
        self.file.write_all(&bytes)?;
        self.file.flush()
    }

    /// Returns the path of this WAL file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads all valid entries from a WAL file. Truncates a corrupt tail if detected.
    ///
    /// Per PRD §14.6: "If WAL is corrupt at the tail, truncate to last good record."
    ///
    /// # Errors
    ///
    /// Returns an IO error if the file cannot be opened.
    pub fn replay(path: impl AsRef<Path>) -> io::Result<Vec<WalEntry>> {
        let path = path.as_ref();
        let mut file = match File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };

        let mut entries = Vec::new();
        let mut last_good_pos: u64 = 0;

        loop {
            let pos = file.stream_position()?;
            match read_one_entry(&mut file) {
                Ok(Some(entry)) => {
                    last_good_pos = file.stream_position()?;
                    entries.push(entry);
                }
                Ok(None) => break, // clean EOF
                Err(_) => {
                    // Corrupt tail — truncate to last known-good position.
                    // Log a structured warning via eprintln so the caller can observe it
                    // without requiring a tracing subscriber to be configured.
                    eprintln!(
                        "wal.recovery: truncating corrupt tail at byte offset {pos}; \
                         last good offset {last_good_pos}"
                    );
                    let f = OpenOptions::new().write(true).open(path)?;
                    f.set_len(last_good_pos)?;
                    break;
                }
            }
        }

        Ok(entries)
    }

    /// Scans an existing WAL file and returns the sequence number of the last valid entry.
    fn find_last_seq(path: &Path) -> io::Result<u64> {
        let entries = Self::replay(path)?;
        Ok(entries.last().map_or(0, |e| e.seq))
    }
}

/// Reads a single length-prefixed bincode entry from `reader`.
///
/// Returns `Ok(None)` on clean EOF (zero bytes read for the length prefix).
fn read_one_entry(reader: &mut impl Read) -> io::Result<Option<WalEntry>> {
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)?;
    let entry: WalEntry = bincode::deserialize(&payload)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(Some(entry))
}

/// Utilities for saving and loading periodic book snapshots.
///
/// Snapshot files are named `snapshot_<seq>.bin` and allow the engine to skip replaying
/// WAL entries older than the snapshot (PRD §14.6).
pub struct Snapshot;

impl Snapshot {
    /// Serialises `book` at WAL sequence `seq` to `path` using bincode.
    ///
    /// # Errors
    ///
    /// Returns an IO or serialisation error.
    pub fn save(path: impl AsRef<Path>, seq: u64, book: &OrderBook) -> io::Result<()> {
        let bytes = bincode::serialize(&(seq, book))
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let mut file = File::create(path)?;
        file.write_all(&bytes)?;
        file.flush()
    }

    /// Loads a snapshot from `path`.
    ///
    /// Returns `(seq, book)` where `seq` is the WAL sequence at snapshot time.
    /// Callers should replay WAL entries with `seq > returned_seq`.
    ///
    /// # Errors
    ///
    /// Returns an IO or deserialisation error.
    pub fn load(path: impl AsRef<Path>) -> io::Result<(u64, OrderBook)> {
        let mut file = File::open(path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let (seq, book): (u64, OrderBook) = bincode::deserialize(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok((seq, book))
    }
}
