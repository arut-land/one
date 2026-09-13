//! Separate durability ports for facts, content-addressed blobs, and key-value data.
//!
//! [`FactLog::commit`] refreshes, deduplicates, and appends under one lock or
//! transaction. Facts and snapshots are Protobuf rows. Compaction requires a
//! snapshot and retains outcomes for retries.
//!
//! Memory implements all three ports for tests and wasm. The native `redb`
//! feature supplies transactional fact logs and key-value tables in one exclusively
//! owned database file. Both implementations share conformance tests.

mod memory;
#[cfg(feature = "redb")]
mod redb;
pub use memory::{MemoryLog, MemoryStore};
#[cfg(feature = "redb")]
pub use redb::{Redb, RedbLog};

/// Facts and snapshots are Protobuf rows, as every persisted contract is.
pub trait Fact: prost::Message + Default + Clone + 'static {}
impl<T: prost::Message + Default + Clone + 'static> Fact for T {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record<F> {
    pub sequence: u64,
    pub epoch: u64,
    pub command_id: String,
    pub fact: F,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub sequence: u64,
    pub epoch: u64,
    pub data: Vec<u8>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageError {
    Io(std::io::ErrorKind),
    Corrupt,
    Conflict { actual: u64 },
    Epoch { current: u64 },
    CursorUnavailable { through: u64 },
    SnapshotRequired,
}
impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for StorageError {}
impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.kind())
    }
}
impl From<prost::DecodeError> for StorageError {
    fn from(_: prost::DecodeError) -> Self {
        Self::Corrupt
    }
}
pub type Result<T> = std::result::Result<T, StorageError>;

/// A synchronous decision over the current sequence, unread facts, and retry outcome.
pub type CommitDecision<'a, F> =
    dyn FnMut(u64, &[Record<F>], Option<Record<F>>) -> Result<Option<F>> + 'a;

pub trait FactLog<F: Fact>: Send + Sync {
    /// Atomically compare the cursor, fence stale epochs, deduplicate, and append.
    fn append(&self, expected: u64, epoch: u64, command_id: &str, fact: F) -> Result<Record<F>> {
        let mut fact = Some(fact);
        let mut duplicate = None;
        let appended = self.commit(None, epoch, command_id, &mut |actual, _, prior| {
            if prior.is_some() {
                duplicate = prior;
                Ok(None)
            } else if expected != actual {
                Err(StorageError::Conflict { actual })
            } else {
                Ok(fact.take())
            }
        })?;
        appended.or(duplicate).ok_or(StorageError::Corrupt)
    }
    /// Refresh, inspect a retry, and optionally append under one storage lock.
    /// `decide` runs once after fencing and cursor validation; it must not
    /// re-enter this log. Return `None` to leave the log unchanged. A `None`
    /// cursor skips refresh, as used by `append`.
    fn commit(
        &self,
        cursor: Option<u64>,
        epoch: u64,
        command_id: &str,
        decide: &mut CommitDecision<'_, F>,
    ) -> Result<Option<Record<F>>>;
    fn outcome_of(&self, command_id: &str) -> Result<Option<Record<F>>>;
    /// Reads records strictly after the acknowledged cursor.
    fn read_from(&self, cursor: u64) -> Result<Vec<Record<F>>>;
    fn snapshot(&self) -> Result<Option<Snapshot>>;
    fn save_snapshot(&self, snapshot: Snapshot) -> Result<()>;
    fn compact(&self, through: u64) -> Result<()>;
}
pub trait BlobStore: Send + Sync {
    fn put_blob(&self, bytes: &[u8]) -> Result<String>;
    /// Rejects anything that is not a BLAKE3 digest; verifies what it returns.
    fn get_blob(&self, digest: &str) -> Result<Option<Vec<u8>>>;
}
pub trait KeyValue: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    fn put(&self, key: &str, value: &[u8]) -> Result<()>;
    fn remove(&self, key: &str) -> Result<()>;
}
/// The address of a blob: BLAKE3, lowercase hex, 64 characters. It is also what
/// `iroh-blobs` addresses by, so the two agree without a translation table.
pub fn digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}
pub(crate) fn checked_digest(id: &str) -> Result<&str> {
    if id.len() == 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(id)
    } else {
        Err(StorageError::Corrupt)
    }
}
