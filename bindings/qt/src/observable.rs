use arut_watch::Subscription;
use core::pin::Pin;
use cxx_qt::{CxxQtThread, Threading};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct ObservableState {
    changes: Arc<Subscription<u64>>,
    active: Arc<AtomicBool>,
}

impl ObservableState {
    pub fn new(changes: Arc<Subscription<u64>>) -> Self {
        Self {
            changes,
            active: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn observe<T>(
        &mut self,
        qt_thread: CxxQtThread<T>,
        refresh: impl Fn(Pin<&mut T>) + Send + Sync + 'static,
    ) where
        T: Threading + 'static,
    {
        let active = Arc::clone(&self.active);
        let pending = Arc::new(AtomicBool::new(false));
        let refresh = Arc::new(refresh);
        self.changes.observe(move |_| {
            if !active.load(Ordering::Acquire) {
                return false;
            }
            if pending.swap(true, Ordering::AcqRel) {
                return true;
            }
            let pending = Arc::clone(&pending);
            let refresh = Arc::clone(&refresh);
            let active = Arc::clone(&active);
            qt_thread
                .queue(move |model| {
                    pending.store(false, Ordering::Release);
                    if active.load(Ordering::Acquire) {
                        refresh(model);
                    }
                })
                .is_ok()
        });
    }
}

impl Drop for ObservableState {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}
