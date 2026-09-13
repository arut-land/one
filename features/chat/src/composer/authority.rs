use super::wire::scope_from_wire;
use super::{ComposerScope, ComposerSnapshot, ReplaceComposer, ReplaceOutcome};
use arut_protocol::chat::composer::v1::ComposerSnapshot as WireSnapshot;
use arut_storage::{KeyValue, MemoryStore, StorageError};
use arut_watch::{Subscription, Watch};
use prost::Message;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub(crate) struct ComposerAuthority {
    inner: Mutex<HashMap<ComposerScope, ScopeState>>,
    store: Arc<dyn KeyValue>,
}
struct ScopeState {
    watch: Watch<ComposerSnapshot>,
    last_command: Option<(String, ComposerSnapshot)>,
}
impl Default for ComposerAuthority {
    fn default() -> Self {
        Self::with_store(Arc::new(MemoryStore::default()))
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PromoteError {
    RevisionConflict(ComposerSnapshot),
    TextMismatch(ComposerSnapshot),
}
fn stored(snapshot: &ComposerSnapshot) -> Vec<u8> {
    WireSnapshot::from(snapshot.clone()).encode_to_vec()
}
fn recovered(snapshot: WireSnapshot) -> Option<ComposerSnapshot> {
    Some(ComposerSnapshot {
        scope: scope_from_wire(snapshot.scope?)?,
        authority_epoch: snapshot.authority_epoch,
        text: snapshot.text,
        revision: snapshot.revision,
    })
}
fn key(scope: &ComposerScope) -> String {
    match scope {
        ComposerScope::Pending(id) => format!("draft:pending:{id}"),
        ComposerScope::Chat(id) => format!("draft:chat:{id}"),
    }
}
impl ComposerAuthority {
    pub fn with_store(store: Arc<dyn KeyValue>) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            store,
        }
    }
    fn load(&self, scope: &ComposerScope) -> Result<ScopeState, StorageError> {
        let snapshot = match self.store.get(&key(scope))? {
            None => ComposerSnapshot::empty(scope.clone()),
            Some(bytes) => recovered(WireSnapshot::decode(&bytes[..])?)
                .filter(|snapshot| &snapshot.scope == scope)
                .ok_or(StorageError::Corrupt)?,
        };
        Ok(ScopeState {
            watch: Watch::new(snapshot),
            last_command: None,
        })
    }
    pub fn snapshot(&self, scope: &ComposerScope) -> Result<ComposerSnapshot, StorageError> {
        let mut scopes = self.inner.lock().unwrap();
        if !scopes.contains_key(scope) {
            scopes.insert(scope.clone(), self.load(scope)?);
        }
        Ok(scopes[scope].watch.get())
    }
    pub fn changes(
        &self,
        scope: &ComposerScope,
    ) -> Result<Arc<Subscription<ComposerSnapshot>>, StorageError> {
        let mut scopes = self.inner.lock().unwrap();
        if !scopes.contains_key(scope) {
            scopes.insert(scope.clone(), self.load(scope)?);
        }
        Ok(scopes[scope].watch.subscribe_values())
    }
    pub fn replace(&self, command: ReplaceComposer) -> Result<ReplaceOutcome, StorageError> {
        let mut scopes = self.inner.lock().unwrap();
        if !scopes.contains_key(&command.scope) {
            scopes.insert(command.scope.clone(), self.load(&command.scope)?);
        }
        let state = scopes.get_mut(&command.scope).unwrap();
        let mut snapshot = state.watch.get();
        if command.authority_epoch != snapshot.authority_epoch {
            return Ok(ReplaceOutcome::AuthorityMismatch {
                current_epoch: snapshot.authority_epoch,
            });
        }
        if let Some((id, snapshot)) = &state.last_command
            && id == &command.command_id
        {
            return Ok(ReplaceOutcome::Applied {
                snapshot: snapshot.clone(),
                duplicate: true,
            });
        }
        if command.base_revision != snapshot.revision {
            return Ok(ReplaceOutcome::RevisionConflict { snapshot });
        }
        snapshot.revision = snapshot
            .revision
            .checked_add(1)
            .ok_or(StorageError::Corrupt)?;
        snapshot.text = command.text;
        self.store.put(&key(&command.scope), &stored(&snapshot))?;
        state.last_command = Some((command.command_id, snapshot.clone()));
        state.watch.set(snapshot.clone());
        Ok(ReplaceOutcome::Applied {
            snapshot,
            duplicate: false,
        })
    }
    /// Repairs recovery state using the revision consumed by a durable start fact.
    pub(crate) fn recover_pending(
        &self,
        pending_id: &str,
        consumed: u64,
    ) -> Result<(), StorageError> {
        let scope = ComposerScope::pending(pending_id);
        let mut scopes = self.inner.lock().unwrap();
        if !scopes.contains_key(&scope) {
            scopes.insert(scope.clone(), self.load(&scope)?);
        }
        let mut snapshot = scopes[&scope].watch.get();
        if snapshot.revision <= consumed {
            snapshot = ComposerSnapshot::empty(scope.clone());
            snapshot.revision = consumed.checked_add(1).ok_or(StorageError::Corrupt)?;
            scopes.insert(
                scope.clone(),
                ScopeState {
                    watch: Watch::new(snapshot.clone()),
                    last_command: None,
                },
            );
        }
        self.store.put(&key(&scope), &stored(&snapshot))
    }

    /// The committed fact records the consumed revision so cleanup can resume after a crash.
    pub(crate) fn promote_pending<T, E: From<StorageError>>(
        &self,
        pending_id: &str,
        revision: u64,
        text: &str,
        chat_id: &str,
        commit: impl FnOnce() -> Result<T, E>,
    ) -> Result<Result<(T, ComposerSnapshot), PromoteError>, E> {
        let scope = ComposerScope::pending(pending_id);
        let mut scopes = self.inner.lock().unwrap();
        if !scopes.contains_key(&scope) {
            scopes.insert(scope.clone(), self.load(&scope)?);
        }
        let pending = scopes[&scope].watch.get();
        if pending.revision != revision {
            return Ok(Err(PromoteError::RevisionConflict(pending)));
        }
        if pending.text != text {
            return Ok(Err(PromoteError::TextMismatch(pending)));
        }
        let mut cleared = ComposerSnapshot::empty(scope.clone());
        cleared.revision = pending
            .revision
            .checked_add(1)
            .ok_or(StorageError::Corrupt)?;
        let result = commit()?;
        let chat_scope = ComposerScope::chat(chat_id);
        let snapshot = ComposerSnapshot::empty(chat_scope.clone());
        scopes.insert(
            scope.clone(),
            ScopeState {
                watch: Watch::new(cleared.clone()),
                last_command: None,
            },
        );
        scopes.insert(
            chat_scope,
            ScopeState {
                watch: Watch::new(snapshot.clone()),
                last_command: None,
            },
        );
        self.store.put(&key(&scope), &stored(&cleared))?;
        Ok(Ok((result, snapshot)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arut_protocol::chat::composer::v1::{
        ComposerService, GetComposerRequest, WatchComposerRequest,
    };
    use arut_rpc::{Code, Request};
    use futures_executor::block_on;

    struct FailedStore;
    impl KeyValue for FailedStore {
        fn get(&self, _: &str) -> Result<Option<Vec<u8>>, StorageError> {
            Err(StorageError::Io(std::io::ErrorKind::PermissionDenied))
        }
        fn put(&self, _: &str, _: &[u8]) -> Result<(), StorageError> {
            Err(StorageError::Corrupt)
        }
        fn remove(&self, _: &str) -> Result<(), StorageError> {
            Err(StorageError::Corrupt)
        }
    }

    #[test]
    fn recovery_failures_cross_rpc_as_status_instead_of_panics() {
        let service = super::super::service::ComposerServiceImpl::new(Arc::new(
            ComposerAuthority::with_store(Arc::new(FailedStore)),
        ));
        let scope = Some(super::super::wire::scope_to_wire(&ComposerScope::pending(
            "pending",
        )));
        assert_eq!(
            block_on(service.get_composer(Request::new(GetComposerRequest {
                scope: scope.clone()
            })))
            .unwrap_err()
            .code,
            Code::Internal
        );
        let result = block_on(service.watch_composer(Request::new(WatchComposerRequest {
            scope,
            after_revision: 0,
        })));
        assert!(matches!(result, Err(error) if error.code == Code::Internal));
    }

    #[test]
    fn a_valid_protobuf_with_the_wrong_scope_is_corruption() {
        let store = Arc::new(MemoryStore::default());
        let scope = ComposerScope::pending("pending");
        store
            .put(
                &key(&scope),
                &stored(&ComposerSnapshot::empty(ComposerScope::chat("other"))),
            )
            .unwrap();
        let authority = ComposerAuthority::with_store(store);
        assert_eq!(authority.snapshot(&scope), Err(StorageError::Corrupt));
        assert!(matches!(
            authority.changes(&scope),
            Err(StorageError::Corrupt)
        ));
    }

    #[test]
    fn subscription_remains_bound_to_its_original_scope_instance() {
        let authority = ComposerAuthority::default();
        let scope = ComposerScope::pending("pending");
        let changes = authority.changes(&scope).unwrap();
        assert_eq!(block_on(changes.changed()).unwrap().revision, 0);
        authority
            .promote_pending("pending", 0, "", "chat", || Ok::<_, StorageError>(()))
            .unwrap()
            .unwrap();
        authority.snapshot(&scope).unwrap();
        assert_eq!(block_on(changes.changed()), None);
    }
}
