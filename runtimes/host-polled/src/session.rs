//! Memory storage and injected host time and identity ports.
use arut_feature_chat::ports::{Clock, Drafts, IdSource, Persist};
use arut_storage::{Fact, FactLog, KeyValue, MemoryLog, MemoryStore, StorageError};
use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, Mutex},
};

/// Named memory logs and draft recovery storage shared by composed features.
/// The map contains storage only. Reopening a namespace with another fact type
/// fails instead of silently creating a different log under the same name.
pub struct MemoryRuntime {
    ids: Arc<dyn IdSource>,
    clock: Arc<dyn Clock + Send + Sync>,
    logs: Mutex<HashMap<String, Arc<dyn Any + Send + Sync>>>,
    drafts: Arc<MemoryStore>,
}
impl MemoryRuntime {
    pub fn new(ids: Arc<dyn IdSource>, clock: Arc<dyn Clock + Send + Sync>) -> Self {
        Self {
            ids,
            clock,
            logs: Mutex::default(),
            drafts: Arc::default(),
        }
    }
}
impl IdSource for MemoryRuntime {
    fn new_id(&self) -> String {
        self.ids.new_id()
    }
}
impl Clock for MemoryRuntime {
    fn now(&self) -> u64 {
        self.clock.now()
    }
}
impl Drafts for MemoryRuntime {
    fn drafts(&self) -> Arc<dyn KeyValue> {
        self.drafts.clone()
    }
}
impl<F: Fact> Persist<F> for MemoryRuntime {
    fn log(&self, namespace: &str) -> Result<Arc<dyn FactLog<F>>, StorageError> {
        self.logs
            .lock()
            .map_err(|_| StorageError::Corrupt)?
            .entry(namespace.into())
            .or_insert_with(|| Arc::new(MemoryLog::<F>::default()))
            .clone()
            .downcast::<MemoryLog<F>>()
            .map(|log| log as Arc<dyn FactLog<F>>)
            .map_err(|_| StorageError::Corrupt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Host;
    impl IdSource for Host {
        fn new_id(&self) -> String {
            "injected".into()
        }
    }
    impl Clock for Host {
        fn now(&self) -> u64 {
            42
        }
    }
    #[test]
    fn namespaces_reopen_without_aliasing_other_logs_or_fact_types() {
        let runtime = MemoryRuntime::new(Arc::new(Host), Arc::new(Host));
        assert_eq!(runtime.new_id(), "injected");
        assert_eq!(runtime.now(), 42);
        let one = <MemoryRuntime as Persist<String>>::log(&runtime, "one").unwrap();
        one.commit(Some(0), 1, "command", &mut |_, _, _| {
            Ok(Some("fact".into()))
        })
        .unwrap();
        assert_eq!(
            <MemoryRuntime as Persist<String>>::log(&runtime, "one")
                .unwrap()
                .read_from(0)
                .unwrap()
                .len(),
            1
        );
        assert!(
            <MemoryRuntime as Persist<String>>::log(&runtime, "two")
                .unwrap()
                .read_from(0)
                .unwrap()
                .is_empty()
        );
        assert!(<MemoryRuntime as Persist<u64>>::log(&runtime, "one").is_err());
    }
}
