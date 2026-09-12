use super::ComposerScope;
use super::service::{scope_from_wire, scope_to_wire};
use crate::ports::IdSource;
use arut_protocol::chat::composer::v1::{
    ComposerServiceClient, ComposerSnapshot as WireSnapshot, GetComposerRequest,
    ReplaceComposerRequest, WatchComposerRequest, replace_composer_response,
};
use arut_rpc::Request;
use arut_watch::{Subscription, Watch};
use futures_util::lock::Mutex as AsyncMutex;
use futures_util::{FutureExt, StreamExt};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerStatus {
    Connecting,
    Synced,
    Failed,
}

#[boltffi::data]
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
    scope: Arc<Watch<ComposerScope>>,
    client_id: String,
    next_command: Arc<AtomicU64>,
    authority_epoch: Arc<AtomicU64>,
    operations: Arc<AsyncMutex<()>>,
}

impl ComposerClient {
    pub(crate) fn new(
        service: ComposerServiceClient,
        scope: ComposerScope,
        ids: Arc<dyn IdSource>,
    ) -> Self {
        Self {
            service,
            state: Arc::new(Watch::new(ComposerState {
                text: String::new(),
                revision: 0,
                status: ComposerStatus::Connecting,
                error: String::new(),
            })),
            scope: Arc::new(Watch::new(scope)),
            client_id: ids.new_id(),
            next_command: Arc::new(AtomicU64::new(1)),
            authority_epoch: Arc::new(AtomicU64::new(1)),
            operations: Arc::new(AsyncMutex::new(())),
        }
    }

    pub fn scope(&self) -> ComposerScope {
        self.scope.get()
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
            Ok(response) => response.message.snapshot.map_or_else(
                || self.apply_error("composer read omitted its snapshot".into()),
                |snapshot| self.apply_snapshot(snapshot, false),
            ),
            Err(error) => self.apply_error(error.to_string()),
        }
    }

    pub async fn replace(&self, text: String) -> ComposerState {
        let _operation = self.operations.lock().await;
        self.replace_unlocked(text).await
    }

    pub(crate) async fn replace_unlocked(&self, text: String) -> ComposerState {
        self.state.update(|state| state.text.clone_from(&text));
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
                Some(replace_composer_response::Outcome::Applied(applied)) => {
                    applied.snapshot.map_or_else(
                        || self.apply_error("composer commit omitted its snapshot".into()),
                        |snapshot| self.apply_snapshot(snapshot, false),
                    )
                }
                Some(replace_composer_response::Outcome::RevisionConflict(conflict)) => {
                    conflict.snapshot.map_or_else(
                        || self.apply_error("composer conflict omitted its snapshot".into()),
                        |snapshot| self.apply_snapshot(snapshot, true),
                    )
                }
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

    /// The host polls this future for the lifetime of the visible composer.
    pub async fn follow(&self) {
        loop {
            let scope_changes = self.scope.subscribe();
            let _ = scope_changes.changed().await;
            let response = self
                .service
                .watch_composer(Request::new(WatchComposerRequest {
                    scope: Some(scope_to_wire(&self.scope())),
                    after_revision: self.state.get().revision,
                }))
                .await;
            let mut stream = match response {
                Ok(response) => response.message,
                Err(error) => {
                    self.apply_error(error.to_string());
                    return;
                }
            };
            loop {
                let next = stream.next().fuse();
                let rebound = scope_changes.changed().fuse();
                futures_util::pin_mut!(next, rebound);
                futures_util::select! {
                    _ = rebound => break,
                    item = next => match item {
                        Some(Ok(response)) => { if let Some(snapshot) = response.snapshot {
                            let _operation = self.operations.lock().await;
                            if snapshot.scope.clone().and_then(scope_from_wire) == Some(self.scope()) { self.apply_snapshot(snapshot, false); }
                        } },
                        Some(Err(error)) => { self.apply_error(error.to_string()); return; },
                        None => break,
                    }
                }
            }
        }
    }

    pub(crate) fn promote(&self, chat_id: &str, snapshot: WireSnapshot) -> Result<(), String> {
        let expected = ComposerScope::chat(chat_id);
        if snapshot.scope.clone().and_then(scope_from_wire) != Some(expected) {
            return Err("start chat composer scope did not match the new chat".into());
        }
        self.scope.set(ComposerScope::chat(chat_id));
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

    fn apply_error(&self, error: String) -> ComposerState {
        self.state.update(|state| {
            state.status = ComposerStatus::Failed;
            state.error = error;
        })
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
        let composer = ComposerClient::new(
            service,
            ComposerScope::pending("account"),
            Arc::new(crate::ports::NativeIds),
        );
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
