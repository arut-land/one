//! Drives only observation callbacks. A callback schedules a native refresh.
use arut_watch::Subscription;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Wake, Waker},
};

struct Observer {
    future: Mutex<Option<Pin<Box<dyn Future<Output = ()> + Send>>>>,
    notified: AtomicBool,
}

impl Wake for Observer {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    /// Polls the observation future to its next await, then again for every
    /// wake that arrived while it was running, so a change is never left
    /// unobserved and the future is never polled twice at once.
    fn wake_by_ref(self: &Arc<Self>) {
        self.notified.store(true, Ordering::Release);
        loop {
            let Ok(mut guard) = self.future.try_lock() else {
                return;
            };
            if !self.notified.swap(false, Ordering::AcqRel) {
                return;
            }
            if let Some(future) = guard.as_mut() {
                let waker = Waker::from(Arc::clone(self));
                if future
                    .as_mut()
                    .poll(&mut Context::from_waker(&waker))
                    .is_ready()
                {
                    *guard = None;
                }
            }
            drop(guard);
            if !self.notified.load(Ordering::Acquire) {
                return;
            }
        }
    }
}

pub(crate) fn observe(
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
    observer.wake_by_ref();
}
