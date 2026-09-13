//! Mutable list-row properties, independent of the widgets GTK recycles.
use gtk::glib;
mod imp {
    use gtk::{glib, prelude::*, subclass::prelude::*};
    use std::cell::{Cell, RefCell};
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::ConversationItem)]
    pub struct ConversationItem {
        #[property(get, set)]
        id: RefCell<String>,
        #[property(get, set)]
        title: RefCell<String>,
        #[property(get, set)]
        unread: Cell<bool>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for ConversationItem {
        const NAME: &'static str = "ArutConversationItem";
        type Type = super::ConversationItem;
    }
    #[glib::derived_properties]
    impl ObjectImpl for ConversationItem {}
}
glib::wrapper! {
    pub struct ConversationItem(ObjectSubclass<imp::ConversationItem>);
}
impl Default for ConversationItem {
    fn default() -> Self {
        glib::Object::new()
    }
}
