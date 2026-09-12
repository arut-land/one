//! # Watch
//!
//! `Watch<T>` wraps one Tokio watch sender containing the value and revision.
//! Subscribers receive the current revision, coalesce intervening updates, and close
//! when the writer is dropped. Tokio enables only `sync`; callers supply polling.
//!
//! The revision is the coalescing key every binding hops a scheduler for, so it
//! moves only when the value actually differs: `update` and `set` compare through
//! `send_if_modified` and leave the revision alone when a write is a no-op.

use futures_lite::{StreamExt, stream::Boxed};
use std::sync::Arc;
use tokio::sync::{Mutex, watch};

#[derive(Clone)]
struct Versioned<T> {
    revision: u64,
    value: T,
}

pub struct Watch<T>(watch::Sender<Versioned<T>>);

pub struct Subscription<T>(Mutex<Boxed<T>>);

impl<T: Send + 'static> Subscription<T> {
    pub async fn changed(&self) -> Option<T> {
        self.0.lock().await.next().await
    }
}

impl<T: Clone + PartialEq + Send + Sync + 'static> Watch<T> {
    pub fn new(value: T) -> Self {
        Self(watch::channel(Versioned { revision: 0, value }).0)
    }

    pub fn get(&self) -> T {
        self.0.borrow().value.clone()
    }

    /// Reads one part of the value without cloning the rest of it.
    ///
    /// The read holds Tokio's internal read lock, so `read` must not call back
    /// into this cell: see the re-entrancy note on [`Watch::update`].
    pub fn read<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        read(&self.0.borrow().value)
    }

    /// Applies `update` to a detached copy and stores it only if it differs.
    ///
    /// Re-entrancy: `update` runs while Tokio's write lock is held, so it must
    /// not touch this cell at all -- `get`, `read`, `set`, `update` and
    /// `subscribe` all take that same lock and would deadlock the caller. The
    /// closure is handed a `&mut T` and needs nothing else; capturing the
    /// `Watch` itself is the mistake this note exists to prevent.
    ///
    /// Clones one candidate. Call `get` separately only when a snapshot is needed.
    pub fn update(&self, update: impl FnOnce(&mut T)) {
        self.0.send_if_modified(|state| {
            let mut next = state.value.clone();
            update(&mut next);
            if next == state.value {
                return false;
            }
            state.revision = state
                .revision
                .checked_add(1)
                .expect("watch revision overflow");
            state.value = next;
            true
        });
    }

    /// Stores `next`, bumping the revision only when it differs from the current
    /// value. Subject to the same re-entrancy rule as [`Watch::update`].
    pub fn set(&self, next: T) -> T {
        let mut result = None;
        self.0.send_if_modified(|state| {
            if state.value == next {
                result = Some(next);
                return false;
            }
            state.value = next;
            state.revision = state
                .revision
                .checked_add(1)
                .expect("watch revision overflow");
            result = Some(state.value.clone());
            true
        });
        result.expect("watch modification runs synchronously")
    }

    pub fn subscribe(&self) -> Arc<Subscription<u64>> {
        let mut receiver = self.0.subscribe();
        // New receivers have already seen the current value. Mark an initial
        // invalidation; subsequent writes may coalesce before it is consumed.
        receiver.mark_changed();
        let stream = futures_lite::stream::unfold(receiver, |mut receiver| async move {
            receiver.changed().await.ok()?;
            let revision = receiver.borrow_and_update().revision;
            Some((revision, receiver))
        });
        Arc::new(Subscription(Mutex::new(Box::pin(stream))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_lite::future::{block_on, poll_once};

    fn ready(changes: &Subscription<u64>) -> Option<u64> {
        block_on(poll_once(changes.changed())).flatten()
    }

    #[test]
    fn coalesces_and_closes_after_the_writer_is_dropped() {
        let watch = Watch::new(0);
        let changes = watch.subscribe();
        assert_eq!(ready(&changes), Some(0));
        for n in 1..=100 {
            watch.set(n);
        }
        assert_eq!(ready(&changes), Some(100));
        assert_eq!(ready(&changes), None);
        drop(watch);
        assert_eq!(block_on(changes.changed()), None);
    }

    #[test]
    fn an_unchanged_write_leaves_the_revision_and_the_subscribers_alone() {
        let watch = Watch::new(String::from("draft"));
        let changes = watch.subscribe();
        assert_eq!(ready(&changes), Some(0));

        assert_eq!(watch.set("draft".into()), "draft");
        watch.update(|value| value.truncate(5));
        assert_eq!(watch.get(), "draft");
        assert_eq!(ready(&changes), None);

        watch.update(|value| value.push('s'));
        assert_eq!(watch.get(), "drafts");
        assert_eq!(ready(&changes), Some(1));
    }

    #[test]
    fn update_clones_once_and_a_panicking_edit_leaves_the_value_unchanged() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct Counted(u64, Arc<AtomicUsize>);
        impl Clone for Counted {
            fn clone(&self) -> Self {
                self.1.fetch_add(1, Ordering::Relaxed);
                Self(self.0, self.1.clone())
            }
        }
        impl PartialEq for Counted {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }
        let clones = Arc::new(AtomicUsize::new(0));
        let watch = Watch::new(Counted(0, clones.clone()));
        watch.update(|value| value.0 = 1);
        assert_eq!(clones.load(Ordering::Relaxed), 1);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            watch.update(|value| {
                value.0 = 2;
                panic!("cancel edit");
            });
        }));
        assert!(result.is_err());
        assert_eq!(watch.read(|value| value.0), 1);
    }

    #[test]
    fn reads_one_field_without_cloning_the_whole_value() {
        let watch = Watch::new((7u64, vec![1u8, 2, 3]));
        assert_eq!(watch.read(|value| value.0), 7);
    }
}
