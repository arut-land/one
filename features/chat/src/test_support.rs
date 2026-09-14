//! Memory ports and service doubles for tests in core crates, over the
//! production `compose` entry point.
use crate::ports::{Clock, Drafts, IdSource, Persist};
use arut_protocol::chat::v1::ChatFact;
use arut_storage::{FactLog, KeyValue, MemoryLog, MemoryStore, StorageError};
use std::sync::Arc;
use std::sync::atomic::Ordering;

#[cfg(test)]
pub(crate) use doubles::{FaultyStore, Intercept};

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

/// Canonical UUIDv7-shaped identities and a fixed acceptance clock for tests.
pub struct TestIds;
impl IdSource for TestIds {
    fn new_id(&self) -> String {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        format!(
            "00000000-0000-7000-8000-{:012x}",
            NEXT.fetch_add(1, Ordering::Relaxed)
        )
    }
}
impl Clock for TestIds {
    fn now(&self) -> u64 {
        123
    }
}

/// Service and port doubles for the failure paths the real implementations
/// cannot be asked for.
#[cfg(test)]
mod doubles {
    use crate::composer::ComposerServiceImpl;
    use arut_protocol::chat::composer::v1::{
        ComposerService, GetComposerRequest, GetComposerResponse, ReplaceComposerRequest,
        ReplaceComposerResponse, WatchComposerRequest, WatchComposerResponse,
    };
    use arut_rpc::{Request, Response, RpcFuture, RpcStream};
    use arut_storage::{KeyValue, MemoryStore, StorageError};
    use std::sync::atomic::Ordering;

    /// A draft store that fails on demand, for recovery paths that only appear
    /// when storage refuses.
    #[derive(Default)]
    pub(crate) struct FaultyStore {
        data: MemoryStore,
        reads: std::sync::atomic::AtomicBool,
        writes: std::sync::atomic::AtomicBool,
    }

    impl FaultyStore {
        /// Every read and write fails until [`FaultyStore::repair`].
        pub(crate) fn fail(&self) {
            self.reads.store(true, Ordering::Relaxed);
            self.writes.store(true, Ordering::Relaxed);
        }
        /// Only writes fail, so recovery still reads what was stored before.
        pub(crate) fn fail_writes(&self) {
            self.writes.store(true, Ordering::Relaxed);
        }
        pub(crate) fn repair(&self) {
            self.reads.store(false, Ordering::Relaxed);
            self.writes.store(false, Ordering::Relaxed);
        }
        fn refuses(flag: &std::sync::atomic::AtomicBool) -> bool {
            flag.load(Ordering::Relaxed)
        }
    }

    impl KeyValue for FaultyStore {
        fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
            if Self::refuses(&self.reads) {
                return Err(StorageError::Io(std::io::ErrorKind::PermissionDenied));
            }
            self.data.get(key)
        }
        fn put(&self, key: &str, value: &[u8]) -> Result<(), StorageError> {
            if Self::refuses(&self.writes) {
                return Err(StorageError::Io(std::io::ErrorKind::StorageFull));
            }
            self.data.put(key, value)
        }
        fn remove(&self, key: &str) -> Result<(), StorageError> {
            if Self::refuses(&self.writes) {
                return Err(StorageError::Corrupt);
            }
            self.data.remove(key)
        }
    }

    /// The composer service with one or two methods answered by a test instead.
    ///
    /// Each hook is handed the real service, so it can refuse, delay, count, or
    /// delegate; the methods without a hook forward unchanged.
    pub(crate) struct Intercept {
        inner: ComposerServiceImpl,
        replace: Option<ReplaceHook>,
        watch: Option<WatchHook>,
    }

    type ReplaceHook = Box<
        dyn Fn(
                &ComposerServiceImpl,
                Request<ReplaceComposerRequest>,
            ) -> RpcFuture<Response<ReplaceComposerResponse>>
            + Send
            + Sync,
    >;
    type WatchHook = Box<
        dyn Fn(
                &ComposerServiceImpl,
                Request<WatchComposerRequest>,
            ) -> RpcFuture<Response<RpcStream<WatchComposerResponse>>>
            + Send
            + Sync,
    >;

    impl Intercept {
        pub(crate) fn new(inner: ComposerServiceImpl) -> Self {
            Self {
                inner,
                replace: None,
                watch: None,
            }
        }
        pub(crate) fn on_replace(
            mut self,
            hook: impl Fn(
                &ComposerServiceImpl,
                Request<ReplaceComposerRequest>,
            ) -> RpcFuture<Response<ReplaceComposerResponse>>
            + Send
            + Sync
            + 'static,
        ) -> Self {
            self.replace = Some(Box::new(hook));
            self
        }
        pub(crate) fn on_watch(
            mut self,
            hook: impl Fn(
                &ComposerServiceImpl,
                Request<WatchComposerRequest>,
            ) -> RpcFuture<Response<RpcStream<WatchComposerResponse>>>
            + Send
            + Sync
            + 'static,
        ) -> Self {
            self.watch = Some(Box::new(hook));
            self
        }
    }

    impl ComposerService for Intercept {
        fn get_composer(
            &self,
            request: Request<GetComposerRequest>,
        ) -> RpcFuture<Response<GetComposerResponse>> {
            self.inner.get_composer(request)
        }
        fn replace_composer(
            &self,
            request: Request<ReplaceComposerRequest>,
        ) -> RpcFuture<Response<ReplaceComposerResponse>> {
            match &self.replace {
                Some(hook) => hook(&self.inner, request),
                None => self.inner.replace_composer(request),
            }
        }
        fn watch_composer(
            &self,
            request: Request<WatchComposerRequest>,
        ) -> RpcFuture<Response<RpcStream<WatchComposerResponse>>> {
            match &self.watch {
                Some(hook) => hook(&self.inner, request),
                None => self.inner.watch_composer(request),
            }
        }
    }
}
