use crate::log::{LogStore, commit_policy, compaction_allowed, snapshot_allowed};
use crate::{
    BlobStore, Fact, FactLog, KeyValue, Record, Result, Snapshot, StorageError, checked_digest,
    digest,
};
use std::{
    collections::BTreeMap,
    sync::{Mutex, MutexGuard},
};
struct LogState<F> {
    records: Vec<Record<F>>,
    snapshot: Option<Snapshot>,
    compacted: u64,
}
pub struct MemoryLog<F>(Mutex<LogState<F>>);
impl<F> Default for MemoryLog<F> {
    fn default() -> Self {
        Self(Mutex::new(LogState {
            records: vec![],
            snapshot: None,
            compacted: 0,
        }))
    }
}
impl<F> MemoryLog<F> {
    fn state(&self) -> Result<MutexGuard<'_, LogState<F>>> {
        self.0.lock().map_err(|_| StorageError::Corrupt)
    }
}
impl<F: Fact> LogStore<F> for MutexGuard<'_, LogState<F>> {
    fn head(&mut self) -> Result<(u64, u64)> {
        Ok(self
            .records
            .last()
            .map_or((0, 1), |record| (record.sequence, record.epoch)))
    }
    fn watermark(&mut self) -> Result<u64> {
        Ok(self.compacted)
    }
    fn duplicate(&mut self, command_id: &str) -> Result<Option<Record<F>>> {
        Ok(self
            .records
            .iter()
            .find(|record| record.command_id == command_id)
            .cloned())
    }
    fn after(&mut self, cursor: u64) -> Result<Vec<Record<F>>> {
        let start = self.records.partition_point(|r| r.sequence <= cursor);
        Ok(self.records[start..].to_vec())
    }
    fn push(&mut self, record: &Record<F>) -> Result<()> {
        self.records.push(record.clone());
        Ok(())
    }
    fn finish(self) -> Result<()> {
        Ok(())
    }
}
impl<F: Fact> FactLog<F> for MemoryLog<F> {
    fn commit(
        &self,
        cursor: Option<u64>,
        epoch: u64,
        id: &str,
        decide: &mut crate::CommitDecision<'_, F>,
    ) -> Result<Option<Record<F>>> {
        commit_policy(self.state()?, cursor, epoch, id, decide)
    }
    fn outcome_of(&self, id: &str) -> Result<Option<Record<F>>> {
        self.state()?.duplicate(id)
    }
    fn read_from(&self, cursor: u64) -> Result<Vec<Record<F>>> {
        let mut state = self.state()?;
        if cursor < state.compacted {
            return Err(StorageError::CursorUnavailable {
                through: state.compacted,
            });
        }
        state.after(cursor)
    }
    fn snapshot(&self) -> Result<Option<Snapshot>> {
        Ok(self.state()?.snapshot.clone())
    }
    fn save_snapshot(&self, snapshot: Snapshot) -> Result<()> {
        let mut state = self.state()?;
        let newest = state.records.last().map_or(0, |r| r.sequence);
        snapshot_allowed(&snapshot, newest, state.compacted)?;
        state.snapshot = Some(snapshot);
        Ok(())
    }
    fn compact(&self, through: u64) -> Result<()> {
        let mut state = self.state()?;
        compaction_allowed(state.snapshot.as_ref(), through)?;
        state.compacted = state.compacted.max(through);
        Ok(())
    }
}
#[derive(Default)]
struct MemoryState {
    values: BTreeMap<String, Vec<u8>>,
    blobs: BTreeMap<String, Vec<u8>>,
}
#[derive(Default)]
pub struct MemoryStore(Mutex<MemoryState>);
impl MemoryStore {
    fn state(&self) -> Result<MutexGuard<'_, MemoryState>> {
        self.0.lock().map_err(|_| StorageError::Corrupt)
    }
}
impl KeyValue for MemoryStore {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.state()?.values.get(key).cloned())
    }
    fn put(&self, key: &str, value: &[u8]) -> Result<()> {
        self.state()?.values.insert(key.to_owned(), value.to_vec());
        Ok(())
    }
    fn remove(&self, key: &str) -> Result<()> {
        self.state()?.values.remove(key);
        Ok(())
    }
}
impl BlobStore for MemoryStore {
    fn put_blob(&self, bytes: &[u8]) -> Result<String> {
        let id = digest(bytes);
        self.state()?.blobs.insert(id.clone(), bytes.to_vec());
        Ok(id)
    }
    fn get_blob(&self, id: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.state()?.blobs.get(checked_digest(id)?).cloned())
    }
}
