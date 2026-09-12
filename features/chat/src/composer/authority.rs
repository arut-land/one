use super::service::scope_from_wire;
use super::{ComposerScope, ComposerSnapshot, ReplaceComposer, ReplaceOutcome};
use arut_protocol::chat::composer::v1::ComposerSnapshot as WireSnapshot;
use arut_storage::{KeyValue, MemoryStore, StorageError};
use arut_watch::{Subscription, Watch};
use prost::Message;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub struct ComposerAuthority {
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
pub enum PromoteError {
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
        let snapshot = self
            .store
            .get(&key(scope))?
            .map(|bytes| WireSnapshot::decode(&bytes[..]))
            .transpose()?
            .and_then(recovered)
            .filter(|snapshot: &ComposerSnapshot| &snapshot.scope == scope)
            .unwrap_or_else(|| ComposerSnapshot::empty(scope.clone()));
        Ok(ScopeState {
            watch: Watch::new(snapshot),
            last_command: None,
        })
    }
    pub fn snapshot(&self, scope: &ComposerScope) -> ComposerSnapshot {
        self.try_snapshot(scope).expect("read composer recovery")
    }
    pub fn try_snapshot(&self, scope: &ComposerScope) -> Result<ComposerSnapshot, StorageError> {
        let mut scopes = self.inner.lock().unwrap();
        if !scopes.contains_key(scope) {
            scopes.insert(scope.clone(), self.load(scope)?);
        }
        Ok(scopes[scope].watch.get())
    }
    pub fn changes(&self, scope: &ComposerScope) -> Arc<Subscription<u64>> {
        self.snapshot(scope);
        self.inner.lock().unwrap()[scope].watch.subscribe()
    }
    pub fn replace(&self, command: ReplaceComposer) -> ReplaceOutcome {
        self.try_replace(command).expect("recover composer")
    }
    pub fn try_replace(&self, command: ReplaceComposer) -> Result<ReplaceOutcome, StorageError> {
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
        snapshot.revision += 1;
        snapshot.text = command.text;
        self.store.put(&key(&command.scope), &stored(&snapshot))?;
        state.last_command = Some((command.command_id, snapshot.clone()));
        state.watch.set(snapshot.clone());
        Ok(ReplaceOutcome::Applied {
            snapshot,
            duplicate: false,
        })
    }
    /// Commits the caller's fact and rebinds the pending draft in one step.
    pub fn promote_pending<T, E: From<StorageError>>(
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
        let result = commit()?;
        let chat_scope = ComposerScope::chat(chat_id);
        let snapshot = ComposerSnapshot::empty(chat_scope.clone());
        self.store.put(&key(&chat_scope), &stored(&snapshot))?;
        self.store.remove(&key(&scope))?;
        scopes.remove(&scope);
        scopes.insert(
            chat_scope,
            ScopeState {
                watch: Watch::new(snapshot.clone()),
                last_command: None,
            },
        );
        Ok(Ok((result, snapshot)))
    }
}
