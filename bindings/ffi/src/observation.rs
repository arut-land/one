//! One revision stream per scope, bridged from a watch onto a BoltFFI stream.
//!
//! A revision stream is one future that awaits the next revision and pushes it,
//! so what it needs is not an executor but something that re-polls the future
//! whenever it is woken. `async_task::spawn` with a schedule function that runs
//! the runnable is exactly that: the first poll happens here, every later poll
//! happens on whichever thread woke it, and the task itself holds the
//! re-entrancy and missed-wake rules that a hand-written `Wake` had to state.
//!
//! `async-task` is `no_std`, reaches no bindgen and no executor, and is the
//! second and last crate `bindings/ffi` names from outside the workspace
//! (`arut-dev check`).

use arut_watch::Subscription;
use boltffi::EventSubscription;
use std::sync::Arc;

/// The stream a generated handle returns: a capacity-one event subscription
/// that stops as soon as its reader is gone.
pub(crate) fn subscription(source: Arc<Subscription<u64>>) -> Arc<EventSubscription<u64>> {
    let target = Arc::new(EventSubscription::new(1));
    let weak = Arc::downgrade(&target);
    observe(source, move |revision| {
        let Some(target) = weak.upgrade() else {
            return false;
        };
        if !target.is_active() {
            return false;
        }
        target.push_event(revision);
        true
    });
    target
}

/// Run `callback` for every revision until it answers `false` or the source
/// closes.
fn observe(source: Arc<Subscription<u64>>, callback: impl Fn(u64) -> bool + Send + Sync + 'static) {
    let (runnable, task) = async_task::spawn(
        async move {
            while let Some(revision) = source.changed().await {
                if !callback(revision) {
                    break;
                }
            }
        },
        |runnable: async_task::Runnable| {
            runnable.run();
        },
    );
    task.detach();
    runnable.run();
}
