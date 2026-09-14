use arut_product_session::Subscription;
use gtk::glib;
use std::{future::Future, sync::Arc};

/// Aborts observers and stream followers when their component leaves the UI.
#[derive(Default)]
pub struct Tasks(Vec<glib::JoinHandle<()>>);

impl Tasks {
    pub fn spawn(&mut self, future: impl Future<Output = ()> + 'static) {
        self.0.push(glib::spawn_future_local(future));
    }

    /// One native GLib task per watch. Its reusable waker drives dispatch;
    /// there is no timer, intermediate message queue, or per-signal task.
    /// Subscribe before calling this method. The initial refresh then closes
    /// the read/setup gap even when the subscription has no initial signal.
    pub fn observe(
        &mut self,
        changes: Arc<Subscription<u64>>,
        mut refresh: impl FnMut() + 'static,
    ) {
        refresh();
        self.spawn(async move {
            while changes.changed().await.is_some() {
                refresh();
            }
        });
    }
}

impl Drop for Tasks {
    fn drop(&mut self) {
        for task in self.0.drain(..) {
            task.abort();
        }
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
    /// The composer's presentation state: the two-way draft plus what the
    /// widgets around it derive from the chat and composer projections.
    pub struct ViewState(ObjectSubclass<properties::ViewState>);
}
impl Default for ViewState {
    fn default() -> Self {
        glib::Object::new()
    }
}
