use crate::*;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
#[derive(Clone)]
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
        if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(StorageError::Corrupt("invalid digest".into()));
        }
        let bytes = read_optional(&self.root.join("blobs").join(id))?;
        if bytes.as_ref().is_some_and(|bytes| digest(bytes) != id) {
            return Err(StorageError::Corrupt("blob digest mismatch".into()));
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
            if path.extension().is_some_and(|e| e == "json") {
                records.push(serde_json::from_slice(&fs::read(path)?)?);
            }
        }
        records.sort_by_key(|r| r.sequence);
        Ok(records)
    }
    fn saved(&self) -> Result<Option<Snapshot>> {
        read_optional(&self.root.join("snapshot.json"))?
            .map(|b| serde_json::from_slice(&b).map_err(Into::into))
            .transpose()
    }
    fn compacted(&self) -> Result<u64> {
        Ok(read_optional(&self.root.join("compacted.json"))?
            .map(|b| serde_json::from_slice(&b))
            .transpose()?
            .unwrap_or(0))
    }
    fn outcome(&self, id: &str, records: &[Record<F>]) -> Result<Option<Record<F>>> {
        if let Some(record) = records.iter().find(|r| r.command_id == id) {
            return Ok(Some(record.clone()));
        }
        read_optional(&self.root.join("outcomes").join(digest(id.as_bytes())))?
            .map(|b| serde_json::from_slice(&b).map_err(Into::into))
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
            &self
                .root
                .join("records")
                .join(format!("{:020}.json", record.sequence)),
            &serde_json::to_vec(&record)?,
        )?;
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
            &self.root.join("snapshot.json"),
            &serde_json::to_vec(&snapshot)?,
        )
    }
    fn compact(&self, through: u64) -> Result<()> {
        let _lock = self.lock()?;
        if self.saved()?.is_none_or(|s| s.sequence < through) {
            return Err(StorageError::SnapshotRequired);
        }
        let records = self.records()?;
        for record in records.iter().filter(|r| r.sequence <= through) {
            atomic(
                &self
                    .root
                    .join("outcomes")
                    .join(digest(record.command_id.as_bytes())),
                &serde_json::to_vec(record)?,
            )?;
        }
        atomic(
            &self.root.join("compacted.json"),
            &serde_json::to_vec(&through.max(self.compacted()?))?,
        )?;
        for record in records.iter().filter(|r| r.sequence <= through) {
            fs::remove_file(
                self.root
                    .join("records")
                    .join(format!("{:020}.json", record.sequence)),
            )?;
        }
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
