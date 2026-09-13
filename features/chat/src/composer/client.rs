//! Composer intents and observation over the generated service contract.
use super::ComposerScope;
use super::projection::{ComposerState, ComposerStatus};
use super::wire::{scope_from_wire, scope_to_wire};
use crate::errors::ComposerError;
use crate::ports::IdSource;
use arut_protocol::chat::composer::v1::{
    ComposerServiceClient, ComposerSnapshot as WireSnapshot, GetComposerRequest,
    ReplaceComposerRequest, WatchComposerRequest, replace_composer_response,
};
use arut_rpc::{Cancellation, Request};
use arut_watch::{Subscription, Watch};
use futures_util::lock::Mutex as AsyncMutex;
use futures_util::{FutureExt, StreamExt};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, PartialEq, Eq)]
enum SnapshotKind {
    Current,
    Conflict,
    Promotion,
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
    cancellation: Arc<Cancellation>,
}

impl ComposerClient {
    pub(crate) fn new(
        service: ComposerServiceClient,
        scope: ComposerScope,
        ids: Arc<dyn IdSource>,
        cancellation: Arc<Cancellation>,
    ) -> Self {
        Self {
            service,
            state: Arc::new(Watch::new(ComposerState {
                text: String::new(),
                revision: 0,
                status: ComposerStatus::Connecting,
                error: None,
            })),
            scope: Arc::new(Watch::new(scope)),
            client_id: ids.new_id(),
            next_command: Arc::new(AtomicU64::new(1)),
            authority_epoch: Arc::new(AtomicU64::new(1)),
            operations: Arc::new(AsyncMutex::new(())),
            cancellation,
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
        self.initialize_unlocked().await
    }

    async fn initialize_unlocked(&self) -> ComposerState {
        let response = self
            .service
            .get_composer(Request::new(GetComposerRequest {
                scope: Some(scope_to_wire(&self.scope())),
            }))
            .await;
        match response {
            Ok(response) => response.message.snapshot.map_or_else(
                || self.apply_error(ComposerError::SnapshotMissing),
                |snapshot| self.apply_snapshot(snapshot, SnapshotKind::Current),
            ),
            Err(error) => self.apply_error(error.into()),
        }
    }

    pub async fn replace(&self, text: String) -> ComposerState {
        let _operation = self.operations.lock().await;
        self.replace_unlocked(text).await
    }

    pub(crate) async fn replace_unlocked(&self, text: String) -> ComposerState {
        if self.state().status == ComposerStatus::Connecting {
            let initialized = self.initialize_unlocked().await;
            if initialized.error.is_some() {
                return initialized;
            }
        }
        self.state.update(|state| state.text.clone_from(&text));
        let sequence = self.next_command.fetch_add(1, Ordering::Relaxed);
        let response = self
            .service
            .replace_composer(Request::new(ReplaceComposerRequest {
                scope: Some(scope_to_wire(&self.scope())),
                command_id: format!("{}:{sequence}", self.client_id),
                authority_epoch: self.authority_epoch.load(Ordering::Acquire),
                base_revision: self.state.read(|state| state.revision),
                text,
            }))
            .await;
        match response {
            Ok(response) => match response.message.outcome {
                Some(replace_composer_response::Outcome::Applied(applied)) => {
                    applied.snapshot.map_or_else(
                        || self.apply_error(ComposerError::SnapshotMissing),
                        |snapshot| self.apply_snapshot(snapshot, SnapshotKind::Current),
                    )
                }
                Some(replace_composer_response::Outcome::RevisionConflict(conflict)) => {
                    conflict.snapshot.map_or_else(
                        || self.apply_error(ComposerError::SnapshotMissing),
                        |snapshot| self.apply_snapshot(snapshot, SnapshotKind::Conflict),
                    )
                }
                Some(replace_composer_response::Outcome::AuthorityMismatch(mismatch)) => {
                    self.authority_epoch
                        .store(mismatch.current_epoch, Ordering::Release);
                    self.apply_error(ComposerError::AuthorityChanged {
                        current_epoch: mismatch.current_epoch,
                    })
                }
                None => self.apply_error(ComposerError::OutcomeMissing),
            },
            Err(error) => self.apply_error(error.into()),
        }
    }

    /// Follow while the surface owns this future. Dropping the future cancels
    /// the subscription; cancelling the conversation also stops any pending I/O.
    pub async fn follow(&self) {
        let cancelled = self.cancellation.token();
        let following = self.follow_scopes().fuse();
        let stop = cancelled.cancelled().fuse();
        futures_util::pin_mut!(following, stop);
        futures_util::select_biased! { () = stop => {}, () = following => {} }
    }

    async fn follow_scopes(&self) {
        loop {
            let changes = self.scope.subscribe();
            changes.changed().await;
            let ended = {
                let follow = self.follow_scope().fuse();
                let rebound = changes.changed().fuse();
                futures_util::pin_mut!(follow, rebound);
                futures_util::select! { () = follow => true, _ = rebound => false }
            };
            // A closed stream is not a retry timer. Wait for promotion instead
            // of repeatedly requesting the same scope when a peer closes it.
            if ended {
                changes.changed().await;
            }
        }
    }

    async fn follow_scope(&self) {
        let after_revision = self.state.read(|state| state.revision);
        tracing::debug!(
            stream = "composer",
            after_revision,
            "resuming a composer stream"
        );
        let response = self
            .service
            .watch_composer(Request::new(WatchComposerRequest {
                scope: Some(scope_to_wire(&self.scope())),
                after_revision,
            }))
            .await;
        let mut stream = match response {
            Ok(response) => response.message,
            Err(error) => {
                self.apply_error(error.into());
                return;
            }
        };
        while let Some(response) = stream.next().await {
            match response {
                Ok(response) => {
                    if let Some(snapshot) = response.snapshot {
                        let _operation = self.operations.lock().await;
                        if snapshot.scope.clone().and_then(scope_from_wire) == Some(self.scope()) {
                            self.apply_snapshot(snapshot, SnapshotKind::Current);
                        }
                    }
                }
                Err(error) => {
                    self.apply_error(error.into());
                    return;
                }
            }
        }
    }

    pub(crate) fn promote(
        &self,
        chat_id: &str,
        snapshot: WireSnapshot,
    ) -> Result<(), ComposerError> {
        let expected = ComposerScope::chat(chat_id);
        if snapshot.scope.clone().and_then(scope_from_wire) != Some(expected) {
            return Err(ComposerError::ScopeMismatch);
        }
        self.scope.set(ComposerScope::chat(chat_id));
        self.apply_snapshot(snapshot, SnapshotKind::Promotion);
        Ok(())
    }

    pub(crate) fn operations(&self) -> Arc<AsyncMutex<()>> {
        Arc::clone(&self.operations)
    }

    fn apply_snapshot(&self, snapshot: WireSnapshot, kind: SnapshotKind) -> ComposerState {
        let Some(scope) = snapshot.scope.clone().and_then(scope_from_wire) else {
            return self.apply_error(ComposerError::ScopeMissing);
        };
        if scope != self.scope() {
            return self.apply_error(ComposerError::ScopeMismatch);
        }
        let epoch = self.authority_epoch.load(Ordering::Acquire);
        if kind != SnapshotKind::Promotion
            && (snapshot.authority_epoch < epoch
                || (snapshot.authority_epoch == epoch && snapshot.revision < self.state().revision))
        {
            return self.state();
        }
        self.authority_epoch
            .store(snapshot.authority_epoch, Ordering::Release);
        let current = snapshot.revision;
        self.state.update(|state| {
            state.text = snapshot.text;
            state.revision = snapshot.revision;
            state.status = if kind == SnapshotKind::Conflict {
                ComposerStatus::Failed
            } else {
                ComposerStatus::Synced
            };
            state.error = (kind == SnapshotKind::Conflict)
                .then_some(ComposerError::RevisionConflict { current });
        });
        self.state.get()
    }

    fn apply_error(&self, error: ComposerError) -> ComposerState {
        self.state.update(|state| {
            state.status = ComposerStatus::Failed;
            state.error = Some(error);
        });
        self.state.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composer::authority::ComposerAuthority;
    use crate::composer::service::ComposerServiceImpl;

    #[test]
    fn stale_zero_revision_and_old_epoch_do_not_replace_the_current_draft() {
        let composer = ComposerClient::new(
            ComposerServiceClient::direct(Arc::new(ComposerServiceImpl::new(Arc::new(
                ComposerAuthority::default(),
            )))),
            ComposerScope::chat("chat"),
            Arc::new(crate::test_support::TestIds),
            Arc::new(Cancellation::root()),
        );
        let snapshot = |epoch, revision, text: &str| WireSnapshot {
            scope: Some(scope_to_wire(&ComposerScope::chat("chat"))),
            authority_epoch: epoch,
            revision,
            text: text.into(),
        };
        composer.apply_snapshot(snapshot(2, 5, "kept"), SnapshotKind::Current);
        composer.apply_snapshot(snapshot(2, 0, "stale"), SnapshotKind::Current);
        composer.apply_snapshot(snapshot(1, 99, "old authority"), SnapshotKind::Current);
        assert_eq!(composer.state().text, "kept");
        assert_eq!(composer.authority_epoch.load(Ordering::Acquire), 2);
        composer.apply_snapshot(snapshot(3, 0, "new authority"), SnapshotKind::Current);
        assert_eq!(composer.state().text, "new authority");
    }

    #[test]
    fn promotion_rejects_a_snapshot_for_another_scope_before_rebinding() {
        let service = ComposerServiceClient::direct(Arc::new(ComposerServiceImpl::new(Arc::new(
            ComposerAuthority::default(),
        ))));
        let composer = ComposerClient::new(
            service,
            ComposerScope::pending("account"),
            Arc::new(crate::test_support::TestIds),
            Arc::new(Cancellation::root()),
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

    #[test]
    fn closed_stream_waits_for_promotion_and_pending_request_can_be_cancelled() {
        use arut_protocol::chat::composer::v1::{
            ComposerService, GetComposerResponse, ReplaceComposerResponse, WatchComposerResponse,
        };
        use arut_rpc::{Response, RpcFuture, RpcStream};
        use std::future::Future;
        use std::task::{Context, Poll};

        struct ClosingService(ComposerServiceImpl, Arc<AtomicU64>);
        impl ComposerService for ClosingService {
            fn get_composer(
                &self,
                request: Request<GetComposerRequest>,
            ) -> RpcFuture<Response<GetComposerResponse>> {
                self.0.get_composer(request)
            }
            fn replace_composer(
                &self,
                request: Request<ReplaceComposerRequest>,
            ) -> RpcFuture<Response<ReplaceComposerResponse>> {
                self.0.replace_composer(request)
            }
            fn watch_composer(
                &self,
                _: Request<WatchComposerRequest>,
            ) -> RpcFuture<Response<RpcStream<WatchComposerResponse>>> {
                if self.1.fetch_add(1, Ordering::Relaxed) == 0 {
                    Box::pin(async {
                        Ok(Response::new(
                            Box::pin(futures_util::stream::empty()) as RpcStream<_>
                        ))
                    })
                } else {
                    Box::pin(std::future::pending())
                }
            }
        }
        let requests = Arc::new(AtomicU64::new(0));
        let service = ComposerServiceClient::direct(Arc::new(ClosingService(
            ComposerServiceImpl::new(Arc::new(ComposerAuthority::default())),
            requests.clone(),
        )));
        let cancellation = Arc::new(Cancellation::root());
        let composer = ComposerClient::new(
            service,
            ComposerScope::pending("account"),
            Arc::new(crate::test_support::TestIds),
            cancellation.clone(),
        );
        let mut follow = Box::pin(composer.follow());
        let mut context = Context::from_waker(futures_util::task::noop_waker_ref());
        assert!(follow.as_mut().poll(&mut context).is_pending());
        assert!(follow.as_mut().poll(&mut context).is_pending());
        assert_eq!(requests.load(Ordering::Relaxed), 1);
        composer
            .promote(
                "chat",
                WireSnapshot {
                    scope: Some(scope_to_wire(&ComposerScope::chat("chat"))),
                    authority_epoch: 1,
                    text: String::new(),
                    revision: 0,
                },
            )
            .unwrap();
        assert!(follow.as_mut().poll(&mut context).is_pending());
        assert_eq!(requests.load(Ordering::Relaxed), 2);
        cancellation.cancel();
        assert_eq!(follow.as_mut().poll(&mut context), Poll::Ready(()));
    }

    #[test]
    fn following_ends_when_the_conversation_scope_is_cancelled() {
        let service = ComposerServiceClient::direct(Arc::new(ComposerServiceImpl::new(Arc::new(
            ComposerAuthority::default(),
        ))));
        let cancellation = Arc::new(Cancellation::root());
        let composer = ComposerClient::new(
            service,
            ComposerScope::pending("account"),
            Arc::new(crate::test_support::TestIds),
            cancellation.clone(),
        );

        cancellation.cancel();

        // Without cancellation this future never completes.
        futures_executor::block_on(composer.follow());
    }
}
