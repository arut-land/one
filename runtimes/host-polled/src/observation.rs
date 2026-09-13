//! Drives only observation callbacks. A callback schedules a native refresh.
use arut_watch::Subscription;
use futures_util::{
    future::BoxFuture,
    task::{ArcWake, waker_ref},
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::Context,
};

struct Observer {
    future: Mutex<Option<BoxFuture<'static, ()>>>,
    notified: AtomicBool,
}

impl ArcWake for Observer {
    fn wake_by_ref(this: &Arc<Self>) {
        this.notified.store(true, Ordering::Release);
        loop {
            let Ok(mut guard) = this.future.try_lock() else {
                return;
            };
            if !this.notified.swap(false, Ordering::AcqRel) {
                return;
            }
            if let Some(future) = guard.as_mut() {
                let waker = waker_ref(this);
                if future
                    .as_mut()
                    .poll(&mut Context::from_waker(&waker))
                    .is_ready()
                {
                    *guard = None;
                }
            }
            drop(guard);
            if !this.notified.load(Ordering::Acquire) {
                return;
            }
        }
    }
}

pub fn observe(
    source: Arc<Subscription<u64>>,
    callback: impl Fn(u64) -> bool + Send + Sync + 'static,
) {
    let observer = Arc::new(Observer {
        future: Mutex::new(Some(Box::pin(async move {
            while let Some(revision) = source.changed().await {
                if !callback(revision) {
                    break;
                }
            }
        }))),
        notified: AtomicBool::new(false),
    });
    Observer::wake_by_ref(&observer);
}
