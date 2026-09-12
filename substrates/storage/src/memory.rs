use crate::*;
use std::{collections::BTreeMap, sync::Mutex};
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
impl<F: Fact> FactLog<F> for MemoryLog<F> {
    fn append(&self, expected: u64, epoch: u64, id: &str, fact: F) -> Result<Record<F>> {
        let mut state = self.0.lock().unwrap();
        let head = state.records.last();
        let current_epoch = head.map_or(1, |r| r.epoch);
        if epoch < current_epoch {
            return Err(StorageError::Epoch {
                current: current_epoch,
            });
        }
        if let Some(record) = state.records.iter().find(|r| r.command_id == id) {
            return Ok(record.clone());
        }
        let actual = head.map_or(0, |r| r.sequence);
        if actual != expected {
            return Err(StorageError::Conflict { actual });
        }
        let record = Record {
            sequence: actual + 1,
            epoch,
            command_id: id.into(),
            fact,
        };
        state.records.push(record.clone());
        Ok(record)
    }
    fn outcome_of(&self, id: &str) -> Result<Option<Record<F>>> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .records
            .iter()
            .find(|r| r.command_id == id)
            .cloned())
    }
    fn read_from(&self, cursor: u64) -> Result<Vec<Record<F>>> {
        let state = self.0.lock().unwrap();
        if cursor < state.compacted {
            return Err(StorageError::CursorUnavailable {
                through: state.compacted,
            });
        }
        Ok(state
            .records
            .iter()
            .filter(|r| r.sequence > cursor)
            .cloned()
            .collect())
    }
    fn snapshot(&self) -> Result<Option<Snapshot>> {
        Ok(self.0.lock().unwrap().snapshot.clone())
    }
    fn save_snapshot(&self, snapshot: Snapshot) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        if snapshot.sequence > state.records.last().map_or(0, |r| r.sequence)
            || snapshot.sequence < state.compacted
        {
            return Err(StorageError::SnapshotRequired);
        }
        state.snapshot = Some(snapshot);
        Ok(())
    }
    fn compact(&self, through: u64) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        if state.snapshot.as_ref().is_none_or(|s| s.sequence < through) {
            return Err(StorageError::SnapshotRequired);
        }
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
impl KeyValue for MemoryStore {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.0.lock().unwrap().values.get(key).cloned())
    }
    fn put(&self, key: &str, value: &[u8]) -> Result<()> {
        self.0
            .lock()
            .unwrap()
            .values
            .insert(key.to_owned(), value.to_vec());
        Ok(())
    }
    fn remove(&self, key: &str) -> Result<()> {
        self.0.lock().unwrap().values.remove(key);
        Ok(())
    }
}
impl BlobStore for MemoryStore {
    fn put_blob(&self, bytes: &[u8]) -> Result<String> {
        let id = digest(bytes);
        self.0
            .lock()
            .unwrap()
            .blobs
            .insert(id.clone(), bytes.to_vec());
        Ok(id)
    }
    fn get_blob(&self, id: &str) -> Result<Option<Vec<u8>>> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .blobs
            .get(checked_digest(id)?)
            .cloned())
    }
}
