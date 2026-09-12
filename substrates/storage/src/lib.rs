//! # Storage
//!
//! FactLog appends immutable, sequenced command outcomes and supports cursor reads,
//! snapshots, compaction, deduplication, and epoch fencing. DirectoryLog locks across
//! processes, writes one Protobuf record per append, fsyncs the file, atomically
//! renames it, and fsyncs the directory. Compaction requires a snapshot and retains
//! outcomes for retry deduplication. It never rewrites all state on append.
//!
//! Redb, behind the off-by-default `redb` feature because the wasm core has no
//! filesystem, gives the same contract in one crash-safe file with typed tables
//! and no C dependency, and carries the KeyValue port beside it. Both pass the
//! same conformance suite; a composition root picks one.
//!
//! BlobStore stores BLAKE3-addressed raw bytes and verifies them on read. BLAKE3 is
//! what `iroh-blobs` hashes with, so an address minted here is the address a peer
//! fetches by once that store backs the port. KeyValue stores each small value
//! independently. Directory keys are hashed to prevent path traversal. Memory
//! implementations provide the same ports for in-process sessions.

//! Separate durability contracts for facts, content, and small unordered values.
#[cfg(not(target_arch = "wasm32"))]
mod directory;
mod memory;
#[cfg(feature = "redb")]
mod redb;
#[cfg(not(target_arch = "wasm32"))]
pub use directory::{Directory, DirectoryLog};
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
        Ok(appended.or(duplicate).expect("append decides once"))
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
    if id.len() == 64 && id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(id)
    } else {
        Err(StorageError::Corrupt)
    }
}

/// The on-disk shape of a record; the fact keeps its own Protobuf encoding.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct StoredRecord {
    #[prost(uint64, tag = "1")]
    pub sequence: u64,
    #[prost(uint64, tag = "2")]
    pub epoch: u64,
    #[prost(string, tag = "3")]
    pub command_id: String,
    #[prost(bytes = "vec", tag = "4")]
    pub fact: Vec<u8>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct StoredSnapshot {
    #[prost(uint64, tag = "1")]
    pub sequence: u64,
    #[prost(uint64, tag = "2")]
    pub epoch: u64,
    #[prost(bytes = "vec", tag = "3")]
    pub data: Vec<u8>,
}
impl<F: Fact> From<&Record<F>> for StoredRecord {
    fn from(record: &Record<F>) -> Self {
        Self {
            sequence: record.sequence,
            epoch: record.epoch,
            command_id: record.command_id.clone(),
            fact: record.fact.encode_to_vec(),
        }
    }
}
impl StoredRecord {
    pub(crate) fn into_record<F: Fact>(self) -> Result<Record<F>> {
        Ok(Record {
            sequence: self.sequence,
            epoch: self.epoch,
            command_id: self.command_id,
            fact: F::decode(&self.fact[..])?,
        })
    }
}
impl From<&Snapshot> for StoredSnapshot {
    fn from(snapshot: &Snapshot) -> Self {
        Self {
            sequence: snapshot.sequence,
            epoch: snapshot.epoch,
            data: snapshot.data.clone(),
        }
    }
}
impl From<StoredSnapshot> for Snapshot {
    fn from(stored: StoredSnapshot) -> Self {
        Self {
            sequence: stored.sequence,
            epoch: stored.epoch,
            data: stored.data,
        }
    }
}
