use crate::{
    BlobStore, Fact, FactLog, KeyValue, Record, Result, Snapshot, StorageError, StoredRecord,
    StoredSnapshot, checked_digest, digest,
};
use prost::Message;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
pub struct Directory {
    root: PathBuf,
}
impl Directory {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        for name in ["facts", "blobs", "values"] {
            fs::create_dir_all(root.join(name))?;
        }
        Ok(Self { root })
    }
    pub fn log<F: Fact>(&self, name: &str) -> Result<DirectoryLog<F>> {
        let root = self.root.join("facts").join(digest(name.as_bytes()));
        fs::create_dir_all(root.join("records"))?;
        fs::create_dir_all(root.join("outcomes"))?;
        Ok(DirectoryLog {
            root,
            fact: PhantomData,
        })
    }
}
impl KeyValue for Directory {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        read_optional(&self.root.join("values").join(digest(key.as_bytes())))
    }
    fn put(&self, key: &str, value: &[u8]) -> Result<()> {
        atomic(
            &self.root.join("values").join(digest(key.as_bytes())),
            value,
        )
    }
    fn remove(&self, key: &str) -> Result<()> {
        let path = self.root.join("values").join(digest(key.as_bytes()));
        if path.exists() {
            fs::remove_file(path)?;
            sync(&self.root.join("values"))?;
        }
        Ok(())
    }
}
impl BlobStore for Directory {
    fn put_blob(&self, bytes: &[u8]) -> Result<String> {
        let id = digest(bytes);
        atomic(&self.root.join("blobs").join(&id), bytes)?;
        Ok(id)
    }
    fn get_blob(&self, id: &str) -> Result<Option<Vec<u8>>> {
        let bytes = read_optional(&self.root.join("blobs").join(checked_digest(id)?))?;
        if bytes.as_ref().is_some_and(|bytes| digest(bytes) != id) {
            return Err(StorageError::Corrupt);
        }
        Ok(bytes)
    }
}

pub struct DirectoryLog<F> {
    root: PathBuf,
    fact: PhantomData<F>,
}
impl<F: Fact> DirectoryLog<F> {
    fn lock(&self) -> Result<File> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root.join("lock"))?;
        file.lock()?;
        Ok(file)
    }
    fn records(&self) -> Result<Vec<Record<F>>> {
        let mut records: Vec<Record<F>> = Vec::new();
        for entry in fs::read_dir(self.root.join("records"))? {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "pb") {
                records.push(StoredRecord::decode(&fs::read(path)?[..])?.into_record()?);
            }
        }
        records.sort_by_key(|r| r.sequence);
        Ok(records)
    }
    fn saved(&self) -> Result<Option<Snapshot>> {
        Ok(read_optional(&self.root.join("snapshot.pb"))?
            .map(|bytes| StoredSnapshot::decode(&bytes[..]))
            .transpose()?
            .map(Snapshot::from))
    }
    fn compacted(&self) -> Result<u64> {
        Ok(read_optional(&self.root.join("compacted"))?
            .and_then(|bytes| bytes.try_into().ok())
            .map_or(0, u64::from_be_bytes))
    }
    fn record_path(&self, sequence: u64) -> PathBuf {
        self.root.join("records").join(format!("{sequence:020}.pb"))
    }
    fn outcome(&self, id: &str, records: &[Record<F>]) -> Result<Option<Record<F>>> {
        if let Some(record) = records.iter().find(|r| r.command_id == id) {
            return Ok(Some(record.clone()));
        }
        read_optional(&self.root.join("outcomes").join(digest(id.as_bytes())))?
            .map(|bytes| StoredRecord::decode(&bytes[..])?.into_record())
            .transpose()
    }
}
impl<F: Fact> FactLog<F> for DirectoryLog<F> {
    fn append(&self, expected: u64, epoch: u64, id: &str, fact: F) -> Result<Record<F>> {
        let _lock = self.lock()?;
        let records = self.records()?;
        let snapshot = self.saved()?;
        let actual = records.last().map_or_else(
            || snapshot.as_ref().map_or(0, |s| s.sequence),
            |r| r.sequence,
        );
        let current = records
            .last()
            .map_or_else(|| snapshot.as_ref().map_or(1, |s| s.epoch), |r| r.epoch);
        if epoch < current {
            return Err(StorageError::Epoch { current });
        }
        if let Some(record) = self.outcome(id, &records)? {
            return Ok(record);
        }
        if expected != actual {
            return Err(StorageError::Conflict { actual });
        }
        let record = Record {
            sequence: actual + 1,
            epoch,
            command_id: id.into(),
            fact,
        };
        atomic(
            &self.record_path(record.sequence),
            &StoredRecord::from(&record).encode_to_vec(),
        )?;
        tracing::debug!(sequence = record.sequence, epoch, "fact appended");
        Ok(record)
    }
    fn outcome_of(&self, id: &str) -> Result<Option<Record<F>>> {
        let _lock = self.lock()?;
        self.outcome(id, &self.records()?)
    }
    fn read_from(&self, cursor: u64) -> Result<Vec<Record<F>>> {
        let _lock = self.lock()?;
        let through = self.compacted()?;
        if cursor < through {
            return Err(StorageError::CursorUnavailable { through });
        }
        Ok(self
            .records()?
            .into_iter()
            .filter(|r| r.sequence > cursor)
            .collect())
    }
    fn snapshot(&self) -> Result<Option<Snapshot>> {
        let _lock = self.lock()?;
        self.saved()
    }
    fn save_snapshot(&self, snapshot: Snapshot) -> Result<()> {
        let _lock = self.lock()?;
        let head = self
            .records()?
            .last()
            .map_or(self.compacted()?, |r| r.sequence);
        if snapshot.sequence > head || snapshot.sequence < self.compacted()? {
            return Err(StorageError::SnapshotRequired);
        }
        atomic(
            &self.root.join("snapshot.pb"),
            &StoredSnapshot::from(&snapshot).encode_to_vec(),
        )
    }
    fn compact(&self, through: u64) -> Result<()> {
        let _lock = self.lock()?;
        if self.saved()?.is_none_or(|s| s.sequence < through) {
            return Err(StorageError::SnapshotRequired);
        }
        let records = self.records()?;
        let compacted: Vec<&Record<F>> = records
            .iter()
            .filter(|r| r.sequence <= through)
            .collect::<Vec<_>>();
        for record in &compacted {
            atomic(
                &self
                    .root
                    .join("outcomes")
                    .join(digest(record.command_id.as_bytes())),
                &StoredRecord::from(*record).encode_to_vec(),
            )?;
        }
        atomic(
            &self.root.join("compacted"),
            &through.max(self.compacted()?).to_be_bytes(),
        )?;
        for record in &compacted {
            fs::remove_file(self.record_path(record.sequence))?;
        }
        tracing::debug!(through, "fact log compacted");
        sync(&self.root.join("records"))
    }
}
fn read_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
fn sync(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let temp = path.with_extension(format!(
        "{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temp, path)?;
    sync(path.parent().expect("storage file has a parent"))
}
