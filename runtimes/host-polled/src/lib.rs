//! Memory ports with injected IDs and clock, plus a thread-local [`arut_rpc::LocalSpawner`] over `async_executor::LocalExecutor`.
//!
//! Hosts drive ready tasks through bounded [`HostPolledSpawner::tick`] calls or
//! run a future through [`HostPolledSpawner::block_on`]. No threads, reactor, or
//! wasm-bindgen are required. This port implementation is tested independently;
//! composition roots own the feature list and hand its clients to product sessions.

use arut_rpc::LocalSpawner;
use async_executor::LocalExecutor;
use std::future::Future;
use std::pin::Pin;

/// A `LocalSpawner` over one `LocalExecutor` the host drives.
pub struct HostPolledSpawner {
    executor: LocalExecutor<'static>,
    budget: usize,
}

impl HostPolledSpawner {
    /// Tasks one [`HostPolledSpawner::tick`] will poll before returning.
    pub const DEFAULT_BUDGET: usize = 64;

    pub fn new() -> Self {
        Self::with_budget(Self::DEFAULT_BUDGET)
    }

    /// Panics on a zero budget: a tick that can poll nothing never drains.
    pub fn with_budget(budget: usize) -> Self {
        assert!(budget > 0, "a host-polled tick budget must poll something");
        Self {
            executor: LocalExecutor::new(),
            budget,
        }
    }

    pub fn budget(&self) -> usize {
        self.budget
    }

    /// True while no spawned task is outstanding.
    pub fn is_idle(&self) -> bool {
        self.executor.is_empty()
    }

    /// Polls up to `budget` ready tasks and reports how many ran.
    ///
    /// Zero means nothing was ready: the host should wait for its next callback
    /// rather than spin. A full budget means more work may be ready; the next
    /// tick checks without exceeding the per-call budget.
    pub fn tick(&self) -> usize {
        let mut polled = 0;
        while polled < self.budget && self.executor.try_tick() {
            polled += 1;
        }
        polled
    }

    /// Drives every spawned task until `future` completes.
    ///
    /// For a host that owns a thread of its own, such as the Android foreground
    /// service. It is absent on wasm on purpose: blocking the page's only
    /// thread deadlocks every host callback the core is waiting for.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn block_on<T>(&self, future: impl Future<Output = T>) -> T {
        futures_lite::future::block_on(self.executor.run(future))
    }
}

impl Default for HostPolledSpawner {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalSpawner for HostPolledSpawner {
    fn spawn_local(&self, future: Pin<Box<dyn Future<Output = ()> + 'static>>) {
        self.executor.spawn(future).detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn a_tick_runs_spawned_work_and_reports_an_idle_executor() {
        let spawner = HostPolledSpawner::new();
        assert!(spawner.is_idle());
        let ran = Rc::new(Cell::new(false));
        let flag = ran.clone();
        spawner.spawn_local(Box::pin(async move { flag.set(true) }));

        assert!(!spawner.is_idle());
        assert!(spawner.tick() > 0);
        assert!(ran.get());
        assert_eq!(spawner.tick(), 0);
        assert!(spawner.is_idle());
    }

    #[test]
    fn one_task_cannot_take_more_than_the_budget() {
        let spawner = HostPolledSpawner::with_budget(4);
        let polls = Rc::new(Cell::new(0usize));
        let counter = polls.clone();
        spawner.spawn_local(Box::pin(async move {
            for _ in 0..32 {
                counter.set(counter.get() + 1);
                futures_lite::future::yield_now().await;
            }
        }));

        assert_eq!(spawner.tick(), 4);
        assert_eq!(polls.get(), 4);
        assert!(!spawner.is_idle());
        while spawner.tick() > 0 {}
        assert_eq!(polls.get(), 32);
        assert!(spawner.is_idle());
    }

    #[test]
    fn a_thread_owning_host_drives_the_same_executor_to_completion() {
        let spawner = HostPolledSpawner::new();
        let done = Rc::new(Cell::new(false));
        let flag = done.clone();
        spawner.spawn_local(Box::pin(async move {
            futures_lite::future::yield_now().await;
            flag.set(true);
        }));

        spawner.block_on(async {
            while !done.get() {
                futures_lite::future::yield_now().await;
            }
        });
        assert!(done.get());
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod ids;
mod observation;
mod session;
#[cfg(not(target_arch = "wasm32"))]
pub use ids::{NativeClock, NativeIds};
pub use observation::observe;
pub use session::MemoryRuntime;
