use arut_product_session::Subscription;
use futures_util::{Stream, StreamExt};
use gtk::glib;
use std::{future::Future, sync::Arc};

/// Aborts observers and stream followers when their component leaves the UI.
#[derive(Default)]
pub struct Tasks(Vec<glib::JoinHandle<()>>);

impl Tasks {
    pub fn spawn(&mut self, future: impl Future<Output = ()> + 'static) {
        self.0.push(glib::spawn_future_local(future));
    }
}

impl Drop for Tasks {
    fn drop(&mut self) {
        for task in self.0.drain(..) {
            task.abort();
        }
    }
}

impl Tasks {
    /// One native GLib task per watch. Its reusable waker drives dispatch;
    /// there is no timer, intermediate message queue, or per-signal task.
    /// Subscribe before calling this method. The initial refresh then closes
    /// the read/setup gap even when the subscription has no initial signal.
    pub fn observe(&mut self, changes: Arc<Subscription<u64>>, refresh: impl FnMut() + 'static) {
        self.observe_stream(
            futures_util::stream::unfold(changes, |changes| async move {
                changes.changed().await.map(|revision| (revision, changes))
            }),
            refresh,
        );
    }

    fn observe_stream(
        &mut self,
        changes: impl Stream<Item = u64> + 'static,
        mut refresh: impl FnMut() + 'static,
    ) {
        refresh();
        self.spawn(async move {
            futures_util::pin_mut!(changes);
            while changes.next().await.is_some() {
                refresh();
            }
        });
    }
}

mod properties {
    use gtk::{glib, prelude::*, subclass::prelude::*};
    use std::cell::{Cell, RefCell};

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::ViewState)]
    pub struct ViewState {
        #[property(get, set)]
        draft: RefCell<String>,
        #[property(get, set)]
        status: RefCell<String>,
        #[property(get, set)]
        enabled: Cell<bool>,
        #[property(get, set)]
        can_send: Cell<bool>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for ViewState {
        const NAME: &'static str = "ArutViewState";
        type Type = super::ViewState;
    }
    #[glib::derived_properties]
    impl ObjectImpl for ViewState {}
}

glib::wrapper! {
    pub struct ViewState(ObjectSubclass<properties::ViewState>);
}
impl Default for ViewState {
    fn default() -> Self {
        glib::Object::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        pin::Pin,
        rc::Rc,
        task::{Context, Poll, Waker},
    };

    // A capacity-one source exercises native GLib scheduling independently of
    // GTK and IPC. The production Watch's coalescing is tested in arut-watch.
    #[derive(Clone, Default)]
    struct Invalidations {
        value: Rc<Cell<u64>>,
        dirty: Rc<Cell<bool>>,
        waker: Rc<RefCell<Option<Waker>>>,
    }
    impl Invalidations {
        fn set(&self, value: u64) {
            self.value.set(value);
            self.dirty.set(true);
            if let Some(waker) = self.waker.borrow().as_ref() {
                waker.wake_by_ref();
            }
        }
        fn get(&self) -> u64 {
            self.value.get()
        }
    }
    impl Stream for Invalidations {
        type Item = u64;
        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<u64>> {
            *self.waker.borrow_mut() = Some(cx.waker().clone());
            if self.dirty.replace(false) {
                Poll::Ready(Some(self.get()))
            } else {
                Poll::Pending
            }
        }
    }

    #[test]
    fn string_properties_keep_their_type_across_binding_fanout() {
        use gtk::prelude::*;
        glib::log_set_always_fatal(glib::LogLevels::LEVEL_ERROR | glib::LogLevels::LEVEL_CRITICAL);
        let source = ViewState::default();
        let target = ViewState::default();
        for property in ["draft", "status"] {
            assert_eq!(
                source.find_property(property).unwrap().value_type(),
                String::static_type()
            );
            source
                .bind_property(property, &target, property)
                .bidirectional()
                .sync_create()
                .build();
        }
        source
            .bind_property("draft", &target, "enabled")
            .transform_to(|_, text: String| Some(!text.is_empty()))
            .sync_create()
            .build();
        source
            .bind_property("status", &target, "can-send")
            .transform_to(|_, text: String| Some(!text.is_empty()))
            .sync_create()
            .build();
        for index in 0..100 {
            let text = format!("Draft {index}\nSecond line");
            source.set_draft(text.as_str());
            source.set_status(text.as_str());
            assert_eq!(target.draft(), text);
            assert_eq!(target.status(), text);
            assert!(target.enabled() && target.can_send());
            target.set_draft("");
            target.set_status("");
            assert_eq!(source.draft(), "");
            assert_eq!(source.status(), "");
            assert!(!target.enabled() && !target.can_send());
        }
    }

    #[test]
    fn burst_refreshes_once_and_cancellation_rejects_queued_updates() {
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| {
                let watch = Invalidations::default();
                let reads = Rc::new(Cell::new(0));
                let value = Rc::new(Cell::new(0));
                let mut tasks = Tasks::default();
                tasks.observe_stream(watch.clone(), {
                    let (watch, reads, value) = (watch.clone(), reads.clone(), value.clone());
                    move || {
                        reads.set(reads.get() + 1);
                        value.set(watch.get());
                    }
                });
                while context.pending() {
                    context.iteration(false);
                }
                let initial = reads.get();
                for n in 1..=10_000 {
                    watch.set(n);
                }
                while context.pending() {
                    context.iteration(false);
                }
                assert_eq!(reads.get(), initial + 1);
                assert_eq!(value.get(), 10_000);
                watch.set(10_001);
                drop(tasks);
                while context.pending() {
                    context.iteration(false);
                }
                assert_eq!(value.get(), 10_000);
            })
            .unwrap();
    }

    #[test]
    fn setup_reads_after_subscription_and_properties_bind_without_display() {
        use gtk::prelude::*;
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| {
                let watch = Invalidations::default();
                let changes = watch.clone();
                watch.set(42);
                let state = ViewState::default();
                let target = ViewState::default();
                let _binding = state
                    .bind_property("draft", &target, "draft")
                    .sync_create()
                    .build();
                let mut tasks = Tasks::default();
                tasks.observe_stream(changes, move || state.set_draft(watch.get().to_string()));
                assert_eq!(target.draft(), "42");
                drop(tasks);
                while context.pending() {
                    context.iteration(false);
                }
            })
            .unwrap();
    }
}
