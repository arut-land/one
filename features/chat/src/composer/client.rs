//! Composer intents and observation over the generated service contract.
use super::scope::ComposerScope;
use super::{ComposerState, ComposerStatus};
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
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// The text a person last typed, and how far the node has acknowledged it.
///
/// Writes coalesce here rather than queueing: every edit replaces the one
/// waiting, so a burst of keystrokes costs one flush plus whatever was already
/// in flight. `acknowledged` only ever moves forward, so an edit superseded by
/// a later one is reported as written once that later one is (ADR 0018: the
/// draft is last-writer-wins, not a log).
#[derive(Default)]
struct Edits {
    next: AtomicU64,
    queued: Mutex<Option<(u64, String)>>,
    acknowledged: AtomicU64,
}

impl Edits {
    /// Replaces the waiting edit with `text` and names the edit's sequence.
    fn queue(&self, text: String) -> u64 {
        let sequence = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        *self.lock() = Some((sequence, text));
        sequence
    }
    /// Drops the waiting edit in favour of a write the caller is about to make.
    fn supersede(&self) -> u64 {
        let sequence = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        self.lock().take();
        sequence
    }
    fn take(&self) -> Option<(u64, String)> {
        self.lock().take()
    }
    fn pending(&self) -> bool {
        self.lock().is_some()
    }
    fn latest(&self) -> u64 {
        self.next.load(Ordering::Acquire)
    }
    fn acknowledge(&self, sequence: u64) {
        self.acknowledged.fetch_max(sequence, Ordering::Release);
    }
    fn acknowledged(&self, sequence: u64) -> bool {
        self.acknowledged.load(Ordering::Acquire) >= sequence
    }
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<(u64, String)>> {
        self.queued.lock().expect("composer edits poisoned")
    }
}

#[derive(Clone)]
pub struct ComposerClient {
    service: ComposerServiceClient,
    state: Arc<Watch<ComposerState>>,
    scope: Arc<Watch<ComposerScope>>,
    client_id: String,
    next_command: Arc<AtomicU64>,
    edits: Arc<Edits>,
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
            edits: Arc::default(),
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
                scope: Some((&self.scope()).into()),
            }))
            .await;
        match response {
            Ok(response) => self.snapshot_or(response.message.snapshot, Self::apply_current),
            Err(error) => self.apply_error(error.into()),
        }
    }

    /// Sets the draft to `text` and resolves once the node has acknowledged it.
    ///
    /// `text` reaches [`ComposerState::text`] as this is called, before the
    /// returned future is polled, so a surface binds its text field straight to
    /// the projection and never queues keystrokes of its own. Edits made while
    /// a write is in flight replace each other; the future resolves when a
    /// write carrying this text, or a later one, is acknowledged.
    pub fn replace(&self, text: String) -> impl Future<Output = ComposerState> {
        self.state.update(|state| state.text.clone_from(&text));
        let sequence = self.edits.queue(text);
        self.flush_until(sequence)
    }

    /// Writes every edit made so far, so a caller can act on a settled draft.
    pub(crate) async fn flush(&self) -> ComposerState {
        self.flush_until(self.edits.latest()).await
    }

    async fn flush_until(&self, sequence: u64) -> ComposerState {
        loop {
            if self.edits.acknowledged(sequence) {
                return self.state();
            }
            let _operation = self.operations.lock().await;
            // Nothing waiting means the flush that drained the queue covered
            // this edit: it ran under the lock this call now holds.
            let Some((queued, text)) = self.edits.take() else {
                return self.state();
            };
            let state = self.write(text).await;
            self.edits.acknowledge(queued);
            if queued >= sequence {
                return state;
            }
        }
    }

    /// Writes `text` while the caller holds the operation lock, in place of any
    /// edit still waiting.
    pub(crate) async fn write_locked(&self, text: String) -> ComposerState {
        let sequence = self.edits.supersede();
        let state = self.write(text).await;
        self.edits.acknowledge(sequence);
        state
    }

    async fn write(&self, text: String) -> ComposerState {
        if self.state.read(|state| state.status) == ComposerStatus::Connecting {
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
                scope: Some((&self.scope()).into()),
                command_id: format!("{}:{sequence}", self.client_id),
                authority_epoch: self.authority_epoch.load(Ordering::Acquire),
                base_revision: self.state.read(|state| state.revision),
                text,
            }))
            .await;
        match response {
            Ok(response) => match response.message.outcome {
                Some(replace_composer_response::Outcome::Applied(applied)) => {
                    self.snapshot_or(applied.snapshot, Self::apply_current)
                }
                Some(replace_composer_response::Outcome::RevisionConflict(conflict)) => {
                    self.snapshot_or(conflict.snapshot, Self::apply_conflict)
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
        self.cancellation
            .token()
            .run_until_cancelled(self.follow_scopes())
            .await;
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
                scope: Some((&self.scope()).into()),
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
                        if scope_of(&snapshot) == Some(self.scope()) {
                            self.apply_current(snapshot);
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
        if scope_of(&snapshot) != Some(expected.clone()) {
            return Err(ComposerError::ScopeMismatch);
        }
        self.scope.set(expected);
        self.adopt_promoted(snapshot);
        Ok(())
    }

    pub(crate) fn operations(&self) -> Arc<AsyncMutex<()>> {
        Arc::clone(&self.operations)
    }

    fn snapshot_or(
        &self,
        snapshot: Option<WireSnapshot>,
        apply: fn(&Self, WireSnapshot) -> ComposerState,
    ) -> ComposerState {
        match snapshot {
            Some(snapshot) => apply(self, snapshot),
            None => self.apply_error(ComposerError::SnapshotMissing),
        }
    }

    /// The node's current draft for this scope, ignored when it is older than
    /// what this client already holds.
    fn apply_current(&self, snapshot: WireSnapshot) -> ComposerState {
        if self.is_stale(&snapshot) {
            return self.state();
        }
        self.write_snapshot(snapshot, None)
    }

    /// Someone else's edit landed first; the node's draft replaces this one and
    /// the conflict is reported with the revision that won.
    fn apply_conflict(&self, snapshot: WireSnapshot) -> ComposerState {
        if self.is_stale(&snapshot) {
            return self.state();
        }
        let current = snapshot.revision;
        self.write_snapshot(snapshot, Some(ComposerError::RevisionConflict { current }))
    }

    /// The draft this client already showed, now under the conversation's own
    /// scope. It is never stale: the authority it came from is the new one.
    fn adopt_promoted(&self, snapshot: WireSnapshot) -> ComposerState {
        self.write_snapshot(snapshot, None)
    }

    fn is_stale(&self, snapshot: &WireSnapshot) -> bool {
        let epoch = self.authority_epoch.load(Ordering::Acquire);
        snapshot.authority_epoch < epoch
            || (snapshot.authority_epoch == epoch
                && snapshot.revision < self.state.read(|state| state.revision))
    }

    fn write_snapshot(
        &self,
        snapshot: WireSnapshot,
        error: Option<ComposerError>,
    ) -> ComposerState {
        let Some(scope) = scope_of(&snapshot) else {
            return self.apply_error(ComposerError::ScopeMissing);
        };
        if scope != self.scope() {
            return self.apply_error(ComposerError::ScopeMismatch);
        }
        self.authority_epoch
            .store(snapshot.authority_epoch, Ordering::Release);
        // An edit still waiting is newer than anything the node has answered,
        // so the person keeps seeing their own text until it is written.
        let keep_local = self.edits.pending();
        self.state.update(|state| {
            if !keep_local {
                state.text = snapshot.text;
            }
            state.revision = snapshot.revision;
            state.status = match error {
                Some(_) => ComposerStatus::Failed,
                None => ComposerStatus::Synced,
            };
            state.error = error;
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

fn scope_of(snapshot: &WireSnapshot) -> Option<ComposerScope> {
    snapshot.scope.clone()?.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composer::authority::ComposerAuthority;
    use crate::composer::service::ComposerServiceImpl;
    use crate::test_support::{Intercept, TestIds};
    use arut_protocol::chat::composer::v1::ComposerService;

    fn client(service: ComposerServiceClient, scope: ComposerScope) -> ComposerClient {
        ComposerClient::new(
            service,
            scope,
            Arc::new(TestIds),
            Arc::new(Cancellation::root()),
        )
    }

    fn direct() -> ComposerServiceClient {
        ComposerServiceClient::direct(Arc::new(ComposerServiceImpl::new(Arc::new(
            ComposerAuthority::default(),
        ))))
    }

    #[test]
    fn stale_zero_revision_and_old_epoch_do_not_replace_the_current_draft() {
        let composer = client(direct(), ComposerScope::chat("chat"));
        let snapshot = |epoch, revision, text: &str| WireSnapshot {
            scope: Some((&ComposerScope::chat("chat")).into()),
            authority_epoch: epoch,
            revision,
            text: text.into(),
        };
        composer.apply_current(snapshot(2, 5, "kept"));
        composer.apply_current(snapshot(2, 0, "stale"));
        composer.apply_current(snapshot(1, 99, "old authority"));
        assert_eq!(composer.state().text, "kept");
        assert_eq!(composer.authority_epoch.load(Ordering::Acquire), 2);
        composer.apply_current(snapshot(3, 0, "new authority"));
        assert_eq!(composer.state().text, "new authority");
    }

    #[test]
    fn promotion_rejects_a_snapshot_for_another_scope_before_rebinding() {
        let composer = client(direct(), ComposerScope::pending("account"));
        let snapshot = WireSnapshot {
            scope: Some((&ComposerScope::chat("other")).into()),
            authority_epoch: 1,
            text: String::new(),
            revision: 0,
        };

        assert!(composer.promote("expected", snapshot).is_err());
        assert_eq!(composer.scope(), ComposerScope::pending("account"));
    }

    /// Every edit is echoed at once and only the last one has to reach the node.
    #[test]
    fn rapid_edits_coalesce_behind_one_write_and_the_last_text_wins() {
        let writes = Arc::new(AtomicU64::new(0));
        let counted = writes.clone();
        let service = ComposerServiceClient::direct(Arc::new(
            Intercept::new(ComposerServiceImpl::new(Arc::new(
                ComposerAuthority::default(),
            )))
            .on_replace(move |inner, request| {
                counted.fetch_add(1, Ordering::Relaxed);
                // Stand in for a node that answers later than the next keystroke.
                let answer = inner.replace_composer(request);
                Box::pin(async move {
                    futures_lite::future::yield_now().await;
                    answer.await
                })
            }),
        ));
        let composer = client(service, ComposerScope::chat("chat"));

        let edits: Vec<_> = (1..=100).map(|n| composer.replace(n.to_string())).collect();
        assert_eq!(composer.state().text, "100", "the echo precedes the writes");
        let states = futures_executor::block_on(futures_util::future::join_all(edits));

        assert!(
            writes.load(Ordering::Relaxed) <= 4,
            "one hundred edits took {} writes",
            writes.load(Ordering::Relaxed)
        );
        assert_eq!(composer.state().text, "100");
        assert_eq!(composer.state().error, None);
        // Every edit resolves against the draft the node acknowledged.
        assert_eq!(states.last().unwrap().text, "100");
    }

    #[test]
    fn closed_stream_waits_for_promotion_and_pending_request_can_be_cancelled() {
        use arut_rpc::{Response, RpcStream};
        use std::future::Future;
        use std::task::{Context, Poll};

        let requests = Arc::new(AtomicU64::new(0));
        let counted = requests.clone();
        let service = ComposerServiceClient::direct(Arc::new(
            Intercept::new(ComposerServiceImpl::new(Arc::new(
                ComposerAuthority::default(),
            )))
            .on_watch(move |_, _| {
                if counted.fetch_add(1, Ordering::Relaxed) == 0 {
                    Box::pin(async {
                        Ok(Response::new(
                            Box::pin(futures_util::stream::empty()) as RpcStream<_>
                        ))
                    })
                } else {
                    Box::pin(std::future::pending())
                }
            }),
        ));
        let cancellation = Arc::new(Cancellation::root());
        let composer = ComposerClient::new(
            service,
            ComposerScope::pending("account"),
            Arc::new(TestIds),
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
                    scope: Some((&ComposerScope::chat("chat")).into()),
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
}
