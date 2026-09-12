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
        match fs::remove_file(path) {
            Ok(()) => sync(&self.root.join("values")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
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
        read_optional(&self.root.join("compacted"))?
            .map(|bytes| {
                bytes
                    .try_into()
                    .map(u64::from_be_bytes)
                    .map_err(|_| StorageError::Corrupt)
            })
            .transpose()
            .map(|through| through.unwrap_or(0))
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
    fn commit(
        &self,
        cursor: Option<u64>,
        epoch: u64,
        id: &str,
        decide: &mut crate::CommitDecision<'_, F>,
    ) -> Result<Option<Record<F>>> {
        let _lock = self.lock()?;
        let through = self.compacted()?;
        if cursor.is_some_and(|cursor| cursor < through) {
            return Err(StorageError::CursorUnavailable { through });
        }
        let mut records = self.records()?;
        let snapshot = self.saved()?;
        let actual = records
            .last()
            .map_or(0, |r| r.sequence)
            .max(snapshot.as_ref().map_or(0, |s| s.sequence))
            .max(through);
        let current = records
            .last()
            .map_or(1, |r| r.epoch)
            .max(snapshot.as_ref().map_or(1, |s| s.epoch));
        if epoch < current {
            return Err(StorageError::Epoch { current });
        }
        if cursor.is_some_and(|cursor| cursor > actual) {
            return Err(StorageError::Conflict { actual });
        }
        let duplicate = self.outcome(id, &records)?;
        records.retain(|record| cursor.is_some_and(|cursor| record.sequence > cursor));
        let Some(fact) = decide(actual, &records, duplicate)? else {
            return Ok(None);
        };
        let record = Record {
            sequence: actual.checked_add(1).ok_or(StorageError::Corrupt)?,
            epoch,
            command_id: id.into(),
            fact,
        };
        atomic(
            &self.record_path(record.sequence),
            &StoredRecord::from(&record).encode_to_vec(),
        )?;
        Ok(Some(record))
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
            .map_or(0, |r| r.sequence)
            .max(self.compacted()?);
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
    let parent = path.parent().expect("storage file has a parent");
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    sync(parent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupted_compaction_keeps_the_durable_head_and_epoch() {
        let root = tempfile::tempdir().unwrap();
        let directory = Directory::open(root.path()).unwrap();
        let log = directory.log::<String>("chat").unwrap();
        log.append(0, 1, "first", "one".into()).unwrap();
        log.append(1, 2, "second", "two".into()).unwrap();
        let leftover = fs::read(log.record_path(1)).unwrap();
        log.save_snapshot(Snapshot {
            sequence: 2,
            epoch: 2,
            data: vec![],
        })
        .unwrap();
        log.compact(2).unwrap();
        // A crash after the watermark fsync can leave any subset of old files.
        fs::write(log.record_path(1), leftover).unwrap();
        drop(log);
        let log = directory.log::<String>("chat").unwrap();
        assert_eq!(
            log.append(2, 1, "stale", "bad".into()),
            Err(StorageError::Epoch { current: 2 })
        );
        log.save_snapshot(Snapshot {
            sequence: 2,
            epoch: 2,
            data: vec![],
        })
        .unwrap();
        assert_eq!(
            log.append(2, 2, "third", "three".into()).unwrap().sequence,
            3
        );
        assert_eq!(log.read_from(2).unwrap()[0].fact, "three");
        assert_eq!(log.outcome_of("second").unwrap().unwrap().sequence, 2);
    }

    #[test]
    fn corrupt_watermark_is_not_an_uncompacted_log() {
        let root = tempfile::tempdir().unwrap();
        let directory = Directory::open(root.path()).unwrap();
        let log = directory.log::<String>("chat").unwrap();
        fs::write(log.root.join("compacted"), [1, 2]).unwrap();
        assert_eq!(log.read_from(0), Err(StorageError::Corrupt));
        assert_eq!(
            log.append(0, 1, "first", "one".into()),
            Err(StorageError::Corrupt)
        );
    }
}
