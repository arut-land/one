//! Separate durability contracts for facts, content, and small unordered values.
mod directory;
mod memory;
pub use directory::{Directory, DirectoryLog};
pub use memory::{MemoryLog, MemoryStore};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

pub trait Fact: Clone + Serialize + DeserializeOwned + Send + Sync + 'static {}
impl<T: Clone + Serialize + DeserializeOwned + Send + Sync + 'static> Fact for T {}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Record<F> {
    pub sequence: u64,
    pub epoch: u64,
    pub command_id: String,
    pub fact: F,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snapshot {
    pub sequence: u64,
    pub epoch: u64,
    pub data: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    Io(String),
    Corrupt(String),
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
        Self::Io(e.to_string())
    }
}
impl From<serde_json::Error> for StorageError {
    fn from(e: serde_json::Error) -> Self {
        Self::Corrupt(e.to_string())
    }
}
pub type Result<T> = std::result::Result<T, StorageError>;

pub trait FactLog<F: Fact>: Send + Sync {
    /// Atomically compare the cursor, fence stale epochs, deduplicate, and append.
    fn append(&self, expected: u64, epoch: u64, command_id: &str, fact: F) -> Result<Record<F>>;
    fn outcome_of(&self, command_id: &str) -> Result<Option<Record<F>>>;
    /// Reads records strictly after the acknowledged cursor.
    fn read_from(&self, cursor: u64) -> Result<Vec<Record<F>>>;
    fn snapshot(&self) -> Result<Option<Snapshot>>;
    fn save_snapshot(&self, snapshot: Snapshot) -> Result<()>;
    fn compact(&self, through: u64) -> Result<()>;
}
pub trait BlobStore: Send + Sync {
    fn put_blob(&self, bytes: &[u8]) -> Result<String>;
    fn get_blob(&self, digest: &str) -> Result<Option<Vec<u8>>>;
}
pub trait KeyValue: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    fn put(&self, key: &str, value: &[u8]) -> Result<()>;
    fn remove(&self, key: &str) -> Result<()>;
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
