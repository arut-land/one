//! Immutable keyed transcript rows exposed through GTK's list model contract.
use arut_product_session::chat::ChatMessage;
use gtk::{gio, glib, prelude::*, subclass::prelude::*};

mod imp {
    use gtk::{gio, glib, prelude::*, subclass::prelude::*};
    use std::cell::RefCell;

    #[derive(Default)]
    pub struct Messages(pub RefCell<Vec<glib::BoxedAnyObject>>);
    #[glib::object_subclass]
    impl ObjectSubclass for Messages {
        const NAME: &'static str = "ArutMessages";
        type Type = super::Messages;
        type Interfaces = (gio::ListModel,);
    }
    impl ObjectImpl for Messages {}
    impl ListModelImpl for Messages {
        fn item_type(&self) -> glib::Type {
            glib::BoxedAnyObject::static_type()
        }
        fn n_items(&self) -> u32 {
            self.0.borrow().len() as u32
        }
        fn item(&self, position: u32) -> Option<glib::Object> {
            self.0
                .borrow()
                .get(position as usize)
                .map(|row| row.clone().upcast())
        }
    }
}
glib::wrapper! {
    pub struct Messages(ObjectSubclass<imp::Messages>) @implements gio::ListModel;
}
impl Default for Messages {
    fn default() -> Self {
        glib::Object::new()
    }
}
impl Messages {
    pub fn last_id(&self) -> u64 {
        self.imp()
            .0
            .borrow()
            .last()
            .map_or(0, |row| row.borrow::<ChatMessage>().id)
    }
    /// Drops every row, for a widget tree that is about to show another
    /// conversation.
    pub fn reset(&self) {
        let removed = self.n_items();
        if removed == 0 {
            return;
        }
        self.imp().0.borrow_mut().clear();
        self.items_changed(0, removed, 0);
    }

    pub fn refresh(&self, read_after: impl FnOnce(u64) -> Vec<ChatMessage>) {
        let messages = read_after(self.last_id());
        if messages.is_empty() {
            return;
        }
        let position = self.n_items();
        let count = messages.len() as u32;
        self.imp()
            .0
            .borrow_mut()
            .extend(messages.into_iter().map(glib::BoxedAnyObject::new));
        self.items_changed(position, 0, count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arut_product_session::chat::ChatRole;
    use std::{cell::RefCell, rc::Rc};
    #[test]
    fn keyed_ranges_append_one_notification_and_preserve_objects() {
        let model = Messages::default();
        let events = Rc::new(RefCell::new(Vec::new()));
        model.connect_items_changed({
            let events = events.clone();
            move |_, p, r, a| events.borrow_mut().push((p, r, a))
        });
        let message = |id| ChatMessage {
            id,
            role: ChatRole::User,
            text: id.to_string(),
            accepted_at_ms: 1,
            starts_time_group: id == 4,
            starts_speaker_group: id == 4,
        };
        model.refresh(|cursor| {
            assert_eq!(cursor, 0);
            vec![message(4)]
        });
        let first = model.item(0).unwrap();
        model.refresh(|cursor| {
            assert_eq!(cursor, 4);
            vec![message(7), message(12)]
        });
        model.refresh(|cursor| {
            assert_eq!(cursor, 12);
            vec![]
        });
        assert_eq!(model.item(0).unwrap(), first);
        assert!(model.item(3).is_none());
        assert_eq!(*events.borrow(), [(0, 0, 1), (1, 0, 2)]);
    }
}
