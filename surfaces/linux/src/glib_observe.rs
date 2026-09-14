//! The GTK binding layer: Rust-to-GLib glue that knows nothing about Arut.
//!
//! `bindings/{swift,kotlin,dotnet,typescript}` each own the same two shapes for
//! their ecosystem -- one task per watch, and a cursored append over a keyed
//! row model. A Rust surface reads the projections directly, so its equivalent
//! is written against GLib rather than an FFI handle and lives beside the
//! surface that needs a main loop to attach it to. Nothing in this module
//! names a product type; `app/` composes it.
use arut_product_session::Subscription;
use gtk::{gio, glib, prelude::*, subclass::prelude::*};
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

type Key = Box<dyn Fn(&glib::BoxedAnyObject) -> u64>;

mod imp {
    use gtk::{gio, glib, prelude::*, subclass::prelude::*};
    use std::cell::{Cell, RefCell};

    #[derive(Default)]
    pub struct Rows {
        pub rows: RefCell<Vec<glib::BoxedAnyObject>>,
        pub key: RefCell<Option<super::Key>>,
        pub cursor: Cell<u64>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for Rows {
        const NAME: &'static str = "ArutRows";
        type Type = super::Rows;
        type Interfaces = (gio::ListModel,);
    }
    impl ObjectImpl for Rows {}
    impl ListModelImpl for Rows {
        fn item_type(&self) -> glib::Type {
            glib::BoxedAnyObject::static_type()
        }
        fn n_items(&self) -> u32 {
            self.rows.borrow().len() as u32
        }
        fn item(&self, position: u32) -> Option<glib::Object> {
            self.rows
                .borrow()
                .get(position as usize)
                .map(|row| row.clone().upcast())
        }
    }
}

glib::wrapper! {
    /// Immutable keyed rows exposed through GTK's list model contract, appended
    /// from a projection's cursored read. Objects already in the model are
    /// never rebuilt, so GTK recycles widgets instead of tearing them down.
    pub struct Rows(ObjectSubclass<imp::Rows>) @implements gio::ListModel;
}

impl Rows {
    /// `key` reads the projection row's own key, which is what the next read
    /// continues from.
    pub fn new<T: 'static>(key: impl Fn(&T) -> u64 + 'static) -> Self {
        let rows: Self = glib::Object::new();
        rows.imp()
            .key
            .replace(Some(Box::new(move |row| key(&row.borrow::<T>()))));
        rows
    }

    /// Where this model has read to. Zero means nothing has been read.
    pub fn cursor(&self) -> u64 {
        self.imp().cursor.get()
    }

    /// Drops every row, for a widget tree that is about to show another
    /// conversation.
    pub fn reset(&self) {
        let removed = self.n_items();
        self.imp().cursor.set(0);
        if removed == 0 {
            return;
        }
        self.imp().rows.borrow_mut().clear();
        self.items_changed(0, removed, 0);
    }

    /// Appends everything after the cursor.
    ///
    /// `reaches` is the key the projection says its rows now reach. A value
    /// behind the cursor means the handle underneath was rebound to another
    /// conversation, which leaves nothing to append to: the model resets first
    /// so the next read starts the new transcript from its beginning.
    pub fn refresh<T: 'static>(&self, reaches: u64, read_after: impl FnOnce(u64) -> Vec<T>) {
        if reaches < self.cursor() {
            self.reset();
        }
        let rows = read_after(self.cursor());
        if rows.is_empty() {
            return;
        }
        let position = self.n_items();
        let count = rows.len() as u32;
        let appended: Vec<glib::BoxedAnyObject> =
            rows.into_iter().map(glib::BoxedAnyObject::new).collect();
        if let (Some(key), Some(last)) = (self.imp().key.borrow().as_ref(), appended.last()) {
            self.imp().cursor.set(key(last));
        }
        self.imp().rows.borrow_mut().extend(appended);
        self.items_changed(position, 0, count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell as StdRefCell, rc::Rc};

    #[derive(Debug, PartialEq, Eq)]
    struct Row(u64);

    fn rows() -> Rows {
        Rows::new::<Row>(|row| row.0)
    }

    #[test]
    fn keyed_ranges_append_one_notification_and_preserve_objects() {
        let model = rows();
        let events = Rc::new(StdRefCell::new(Vec::new()));
        model.connect_items_changed({
            let events = events.clone();
            move |_, p, r, a| events.borrow_mut().push((p, r, a))
        });
        model.refresh(4, |cursor| {
            assert_eq!(cursor, 0);
            vec![Row(4)]
        });
        let first = model.item(0).unwrap();
        model.refresh(12, |cursor| {
            assert_eq!(cursor, 4);
            vec![Row(7), Row(12)]
        });
        model.refresh(12, |cursor| {
            assert_eq!(cursor, 12);
            Vec::<Row>::new()
        });
        assert_eq!(model.item(0).unwrap(), first);
        assert!(model.item(3).is_none());
        assert_eq!(*events.borrow(), [(0, 0, 1), (1, 0, 2)]);
    }

    #[test]
    fn a_cursor_that_rewinds_resets_before_reading() {
        let model = rows();
        model.refresh(9, |_| vec![Row(3), Row(9)]);
        assert_eq!((model.cursor(), model.n_items()), (9, 2));
        model.refresh(2, |cursor| {
            assert_eq!(cursor, 0, "a rebound handle has nothing to append to");
            vec![Row(1), Row(2)]
        });
        assert_eq!((model.cursor(), model.n_items()), (2, 2));
    }
}
