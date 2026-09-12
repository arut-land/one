//! Coalesced state observation, with no executor or observer threads.
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

type Callback = Box<dyn Fn(u64) -> bool + Send + Sync>;

struct Notifications {
    revision: watch::Sender<u64>,
    callbacks: Mutex<Vec<Callback>>,
}

/// A subscription starts with the current revision. Intermediate revisions coalesce.
pub struct Subscription<T> {
    receiver: Mutex<watch::Receiver<T>>,
    notifications: Arc<Notifications>,
}

impl Subscription<u64> {
    pub fn try_recv(&self) -> Option<u64> {
        let mut receiver = self.receiver.lock().expect("watch receiver poisoned");
        if receiver.has_changed().unwrap_or(false) {
            Some(*receiver.borrow_and_update())
        } else {
            None
        }
    }

    /// Register a scheduler notification. Return false to detach.
    /// Callbacks must schedule work rather than mutate the watched value.
    pub fn observe(&self, callback: impl Fn(u64) -> bool + Send + Sync + 'static) {
        let mut callbacks = self
            .notifications
            .callbacks
            .lock()
            .expect("watch callbacks poisoned");
        if callback(*self.notifications.revision.borrow()) {
            callbacks.push(Box::new(callback));
        }
    }

    /// Wait on the caller's executor, including a single-threaded wasm executor.
    pub async fn changed(&self) -> Option<u64> {
        let mut receiver = self
            .receiver
            .lock()
            .expect("watch receiver poisoned")
            .clone();
        receiver.changed().await.ok()?;
        let revision = *receiver.borrow_and_update();
        *self.receiver.lock().expect("watch receiver poisoned") = receiver;
        Some(revision)
    }
}

pub struct Watch<T> {
    value: watch::Sender<T>,
    notifications: Arc<Notifications>,
    writer: Mutex<()>,
}

impl<T: Clone> Watch<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: watch::channel(value).0,
            notifications: Arc::new(Notifications {
                revision: watch::channel(0).0,
                callbacks: Mutex::new(Vec::new()),
            }),
            writer: Mutex::new(()),
        }
    }

    pub fn get(&self) -> T {
        self.value.borrow().clone()
    }

    pub fn update(&self, update: impl FnOnce(&mut T)) -> T {
        let _writer = self.writer.lock().expect("watch writer poisoned");
        self.value.send_modify(update);
        let snapshot = self.get();
        self.notifications.revision.send_modify(|revision| {
            *revision = revision.checked_add(1).expect("watch revision overflow");
        });
        let revision = *self.notifications.revision.borrow();
        self.notifications
            .callbacks
            .lock()
            .expect("watch callbacks poisoned")
            .retain(|callback| callback(revision));
        snapshot
    }

    pub fn set(&self, value: T) -> T {
        self.update(|current| *current = value)
    }

    pub fn subscribe(&self) -> Arc<Subscription<u64>> {
        let mut receiver = self.notifications.revision.subscribe();
        receiver.mark_changed();
        Arc::new(Subscription {
            receiver: Mutex::new(receiver),
            notifications: Arc::clone(&self.notifications),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn initial_revision_and_coalesced_updates() {
        let state = Watch::new(0);
        let changes = state.subscribe();
        assert_eq!(changes.try_recv(), Some(0));
        assert_eq!(changes.try_recv(), None);
        for value in 1..=100 {
            state.set(value);
        }
        assert_eq!(changes.try_recv(), Some(100));
        assert_eq!(changes.try_recv(), None);
        assert_eq!(state.get(), 100);
    }

    #[test]
    fn callbacks_receive_invalidations_without_threads() {
        let state = Watch::new(0);
        let observed = Arc::new(AtomicU64::new(0));
        let target = Arc::clone(&observed);
        state.subscribe().observe(move |revision| {
            target.store(revision, Ordering::Relaxed);
            revision < 2
        });
        state.set(1);
        assert_eq!(observed.load(Ordering::Relaxed), 1);
        state.set(2);
        state.set(3);
        assert_eq!(observed.load(Ordering::Relaxed), 2);
    }
}
