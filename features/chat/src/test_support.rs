//! Memory ports for tests in core crates, using the production `compose` entry point.
use crate::ports::{Clock, Drafts, IdSource, Persist};
use arut_protocol::chat::v1::ChatFact;
use arut_storage::{FactLog, KeyValue, MemoryLog, MemoryStore, StorageError};
use std::sync::Arc;

pub struct MemoryPorts {
    ids: Arc<dyn IdSource>,
    log: Arc<MemoryLog<ChatFact>>,
    drafts: Arc<MemoryStore>,
    pub now: u64,
}
impl MemoryPorts {
    pub fn new(ids: Arc<dyn IdSource>) -> Self {
        Self {
            ids,
            log: Arc::default(),
            drafts: Arc::default(),
            now: 0,
        }
    }
}
impl IdSource for MemoryPorts {
    fn new_id(&self) -> String {
        self.ids.new_id()
    }
}
impl Clock for MemoryPorts {
    fn now(&self) -> u64 {
        self.now
    }
}
impl Drafts for MemoryPorts {
    fn drafts(&self) -> Arc<dyn KeyValue> {
        self.drafts.clone()
    }
}
impl Persist<ChatFact> for MemoryPorts {
    fn log(&self, _: &str) -> Result<Arc<dyn FactLog<ChatFact>>, StorageError> {
        Ok(self.log.clone())
    }
}
