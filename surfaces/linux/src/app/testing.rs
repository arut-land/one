//! Widget-tree lookup shared by the review fixtures and the component tests.
use gtk::prelude::*;

pub fn find(root: &gtk::Widget, matches: &impl Fn(&gtk::Widget) -> bool) -> Option<gtk::Widget> {
    if matches(root) {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(found) = find(&widget, matches) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

pub fn descendant<T: IsA<gtk::Widget>>(root: &impl IsA<gtk::Widget>) -> Option<T> {
    find(root.as_ref(), &|widget| widget.is::<T>()).and_then(|widget| widget.downcast().ok())
}

/// The transcript's `GtkListView`, which is the one with the log role.
#[cfg(feature = "review")]
pub fn transcript(root: &impl IsA<gtk::Widget>) -> Option<gtk::ListView> {
    find(root.as_ref(), &|widget| {
        widget.is::<gtk::ListView>() && widget.accessible_role() == gtk::AccessibleRole::Log
    })
    .and_then(|widget| widget.downcast().ok())
}
