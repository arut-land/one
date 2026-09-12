//! A versioned cell over Tokio watch. No executor, queues, or callback registry.
use futures_util::{StreamExt, stream::BoxStream};
use std::sync::Arc;
use tokio::sync::watch;

#[derive(Clone)]
struct Versioned<T> {
    revision: u64,
    value: T,
}

pub struct Watch<T>(watch::Sender<Versioned<T>>);

pub struct Subscription<T>(futures_util::lock::Mutex<BoxStream<'static, T>>);

impl<T: Send + 'static> Subscription<T> {
    pub async fn changed(&self) -> Option<T> {
        self.0.lock().await.next().await
    }

    pub fn try_recv(&self) -> Option<T> {
        use futures_util::FutureExt;
        self.0.try_lock()?.next().now_or_never().flatten()
    }
}

impl<T: Clone + Send + Sync + 'static> Watch<T> {
    pub fn new(value: T) -> Self {
        Self(watch::channel(Versioned { revision: 0, value }).0)
    }

    pub fn get(&self) -> T {
        self.0.borrow().value.clone()
    }

    pub fn update(&self, update: impl FnOnce(&mut T)) -> T {
        let mut result = None;
        self.0.send_modify(|state| {
            update(&mut state.value);
            state.revision = state
                .revision
                .checked_add(1)
                .expect("watch revision overflow");
            result = Some(state.value.clone());
        });
        result.expect("watch modification runs synchronously")
    }

    pub fn set(&self, next: T) -> T {
        self.update(|value| *value = next)
    }

    pub fn subscribe(&self) -> Arc<Subscription<u64>> {
        let mut receiver = self.0.subscribe();
        receiver.mark_changed();
        let stream = futures_util::stream::unfold(receiver, |mut receiver| async move {
            receiver.changed().await.ok()?;
            let revision = receiver.borrow_and_update().revision;
            Some((revision, receiver))
        });
        Arc::new(Subscription(futures_util::lock::Mutex::new(Box::pin(
            stream,
        ))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coalesces_and_closes_after_the_writer_is_dropped() {
        let watch = Watch::new(0);
        let changes = watch.subscribe();
        assert_eq!(changes.try_recv(), Some(0));
        for n in 1..=100 {
            watch.set(n);
        }
        assert_eq!(changes.try_recv(), Some(100));
        assert_eq!(changes.try_recv(), None);
        drop(watch);
        assert_eq!(futures_executor::block_on(changes.changed()), None);
    }
}
