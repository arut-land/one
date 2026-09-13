//! A single-child allocation container: cap reading width and report breakpoints
//! from GTK layout, without a frame callback or resize polling.
use gtk::{glib, prelude::*, subclass::prelude::*};
mod imp {
    use gtk::{glib, prelude::*, subclass::prelude::*};
    use std::cell::{Cell, RefCell};
    type Breakpoint = Box<dyn Fn(bool)>;
    #[derive(Default)]
    pub struct Column {
        pub child: RefCell<Option<gtk::Widget>>,
        pub maximum: Cell<i32>,
        pub narrow: Cell<bool>,
        pub breakpoint: RefCell<Option<Breakpoint>>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for Column {
        const NAME: &'static str = "ArutColumn";
        type Type = super::Column;
        type ParentType = gtk::Widget;
    }
    impl ObjectImpl for Column {
        fn dispose(&self) {
            if let Some(child) = self.child.take() {
                child.unparent();
            }
        }
    }
    impl WidgetImpl for Column {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let Some(child) = self.child.borrow().clone() else {
                return (0, 0, -1, -1);
            };
            let (min, natural, a, b) = child.measure(orientation, for_size);
            if orientation == gtk::Orientation::Horizontal {
                (min.min(320), natural.min(self.maximum.get()), a, b)
            } else {
                (min, natural, a, b)
            }
        }
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let narrow = width < 720;
            if self.narrow.replace(narrow) != narrow
                && let Some(callback) = self.breakpoint.borrow().as_ref()
            {
                callback(narrow);
            }
            if let Some(child) = self.child.borrow().as_ref() {
                let capped = width.min(self.maximum.get());
                child.allocate(
                    capped,
                    height,
                    baseline,
                    Some(
                        gtk::gsk::Transform::new().translate(&gtk::graphene::Point::new(
                            ((width - capped) / 2) as f32,
                            0.0,
                        )),
                    ),
                );
            }
        }
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            if let Some(child) = self.child.borrow().as_ref() {
                self.obj().snapshot_child(child, snapshot);
            }
        }
    }
}
glib::wrapper! {
    pub struct Column(ObjectSubclass<imp::Column>) @extends gtk::Widget, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}
impl Column {
    pub fn new(child: &impl IsA<gtk::Widget>, maximum: i32) -> Self {
        let column: Self = glib::Object::new();
        column.imp().maximum.set(maximum);
        child.as_ref().set_parent(&column);
        column.imp().child.replace(Some(child.as_ref().clone()));
        column.set_hexpand(true);
        column.set_vexpand(true);
        column
    }
    pub fn on_breakpoint(&self, callback: impl Fn(bool) + 'static) {
        self.imp().breakpoint.replace(Some(Box::new(callback)));
    }
}
