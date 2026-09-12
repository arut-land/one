use super::service::{scope_from_wire, scope_to_wire};
use super::{ComposerScope, ComposerSnapshot};
use arut_protocol::chat::composer::v1::{
    ComposerServiceClient, ComposerSnapshot as WireSnapshot, DraftReplaced as WireDraftReplaced,
    GetComposerRequest, ReplaceComposerRequest, WatchComposerRequest, replace_composer_response,
};
use arut_rpc::Request;
use arut_watch::{Subscription, Watch};
use futures_util::lock::Mutex as AsyncMutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerStatus {
    Connecting,
    Synced,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerState {
    pub text: String,
    pub revision: u64,
    pub status: ComposerStatus,
    pub error: String,
}

#[derive(Clone)]
pub struct ComposerClient {
    service: ComposerServiceClient,
    state: Arc<Watch<ComposerState>>,
    scope: Arc<Mutex<ComposerScope>>,
    client_id: String,
    next_command: Arc<AtomicU64>,
    authority_epoch: Arc<AtomicU64>,
    operations: Arc<AsyncMutex<()>>,
}

impl ComposerClient {
    pub(crate) fn new(service: ComposerServiceClient, scope: ComposerScope) -> Self {
        Self {
            service,
            state: Arc::new(Watch::new(ComposerState {
                text: String::new(),
                revision: 0,
                status: ComposerStatus::Connecting,
                error: String::new(),
            })),
            scope: Arc::new(Mutex::new(scope)),
            client_id: Uuid::new_v4().to_string(),
            next_command: Arc::new(AtomicU64::new(1)),
            authority_epoch: Arc::new(AtomicU64::new(1)),
            operations: Arc::new(AsyncMutex::new(())),
        }
    }

    pub fn scope(&self) -> ComposerScope {
        self.scope
            .lock()
            .expect("composer scope lock poisoned")
            .clone()
    }

    pub fn state(&self) -> ComposerState {
        self.state.get()
    }

    pub fn changes(&self) -> Arc<Subscription<u64>> {
        self.state.subscribe()
    }

    pub async fn initialize(&self) -> ComposerState {
        let _operation = self.operations.lock().await;
        let response = self
            .service
            .get_composer(Request::new(GetComposerRequest {
                scope: Some(scope_to_wire(&self.scope())),
            }))
            .await;
        match response {
            Ok(response) => response
                .message
                .snapshot
                .map(|snapshot| self.apply_snapshot(snapshot, false))
                .unwrap_or_else(|| self.apply_error("composer read omitted its snapshot".into())),
            Err(error) => self.apply_error(error.to_string()),
        }
    }

    pub async fn replace(&self, text: String) -> ComposerState {
        let _operation = self.operations.lock().await;
        self.replace_unlocked(text).await
    }

    pub(crate) async fn replace_unlocked(&self, text: String) -> ComposerState {
        let sequence = self.next_command.fetch_add(1, Ordering::Relaxed);
        let response = self
            .service
            .replace_composer(Request::new(ReplaceComposerRequest {
                scope: Some(scope_to_wire(&self.scope())),
                command_id: format!("{}:{sequence}", self.client_id),
                authority_epoch: self.authority_epoch.load(Ordering::Acquire),
                base_revision: self.state.get().revision,
                text,
            }))
            .await;
        match response {
            Ok(response) => match response.message.outcome {
                Some(replace_composer_response::Outcome::Applied(applied)) => applied
                    .snapshot
                    .map(|snapshot| self.apply_snapshot(snapshot, false))
                    .unwrap_or_else(|| {
                        self.apply_error("composer commit omitted its snapshot".into())
                    }),
                Some(replace_composer_response::Outcome::RevisionConflict(conflict)) => conflict
                    .snapshot
                    .map(|snapshot| self.apply_snapshot(snapshot, true))
                    .unwrap_or_else(|| {
                        self.apply_error("composer conflict omitted its snapshot".into())
                    }),
                Some(replace_composer_response::Outcome::AuthorityMismatch(mismatch)) => {
                    self.authority_epoch
                        .store(mismatch.current_epoch, Ordering::Release);
                    self.apply_error(format!(
                        "composer authority changed to epoch {}",
                        mismatch.current_epoch
                    ))
                }
                None => self.apply_error("composer commit omitted its outcome".into()),
            },
            Err(error) => self.apply_error(error.to_string()),
        }
    }

    pub async fn sync_once(&self) -> ComposerState {
        let _operation = self.operations.lock().await;
        let response = self
            .service
            .watch_composer(Request::new(WatchComposerRequest {
                scope: Some(scope_to_wire(&self.scope())),
                after_revision: self.state.get().revision,
            }))
            .await;
        match response {
            Ok(response) => {
                for fact in response.message.facts {
                    self.apply_fact(fact);
                }
                response
                    .message
                    .snapshot
                    .map(|snapshot| self.apply_snapshot(snapshot, false))
                    .unwrap_or_else(|| self.state.get())
            }
            Err(error) => self.apply_error(error.to_string()),
        }
    }

    pub(crate) fn promote(&self, chat_id: &str, snapshot: WireSnapshot) -> Result<(), String> {
        let expected = ComposerScope::chat(chat_id);
        if snapshot.scope.clone().and_then(scope_from_wire) != Some(expected.clone()) {
            return Err("start chat composer scope did not match the new chat".into());
        }
        *self.scope.lock().expect("composer scope lock poisoned") = ComposerScope::chat(chat_id);
        self.apply_snapshot(snapshot, false);
        Ok(())
    }

    pub(crate) fn operations(&self) -> Arc<AsyncMutex<()>> {
        Arc::clone(&self.operations)
    }

    fn apply_snapshot(&self, snapshot: WireSnapshot, conflict: bool) -> ComposerState {
        let Some(scope) = snapshot.scope.clone().and_then(scope_from_wire) else {
            return self.apply_error("composer response omitted its scope".into());
        };
        if scope != self.scope() {
            return self.apply_error("composer response scope did not match this chat".into());
        }
        self.authority_epoch
            .store(snapshot.authority_epoch, Ordering::Release);
        self.state.update(|state| {
            if snapshot.revision >= state.revision || snapshot.revision == 0 {
                state.text = snapshot.text;
                state.revision = snapshot.revision;
            }
            state.status = if conflict {
                ComposerStatus::Failed
            } else {
                ComposerStatus::Synced
            };
            state.error = if conflict {
                "composer draft changed before this edit was committed".into()
            } else {
                String::new()
            };
        })
    }

    fn apply_fact(&self, fact: WireDraftReplaced) {
        if fact
            .scope
            .as_ref()
            .and_then(|scope| scope_from_wire(scope.clone()))
            .as_ref()
            != Some(&self.scope())
        {
            return;
        }
        self.state.update(|state| {
            if fact.revision == state.revision + 1 {
                state.text = fact.text;
                state.revision = fact.revision;
                state.status = ComposerStatus::Synced;
                state.error.clear();
            }
        });
    }

    fn apply_error(&self, error: String) -> ComposerState {
        self.state.update(|state| {
            state.status = ComposerStatus::Failed;
            state.error = error;
        })
    }
}

impl From<WireSnapshot> for ComposerSnapshot {
    fn from(snapshot: WireSnapshot) -> Self {
        Self {
            scope: snapshot
                .scope
                .and_then(scope_from_wire)
                .expect("wire composer snapshot must have a scope"),
            authority_epoch: snapshot.authority_epoch,
            text: snapshot.text,
            revision: snapshot.revision,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composer::authority::ComposerAuthority;
    use crate::composer::service::ComposerServiceImpl;

    #[test]
    fn promotion_rejects_a_snapshot_for_another_scope_before_rebinding() {
        let service = ComposerServiceClient::direct(Arc::new(ComposerServiceImpl::new(Arc::new(
            ComposerAuthority::default(),
        ))));
        let composer = ComposerClient::new(service, ComposerScope::pending("account"));
        let snapshot = WireSnapshot {
            scope: Some(scope_to_wire(&ComposerScope::chat("other"))),
            authority_epoch: 1,
            text: String::new(),
            revision: 0,
        };

        assert!(composer.promote("expected", snapshot).is_err());
        assert_eq!(composer.scope(), ComposerScope::pending("account"));
    }
}
