//! Mutable list-row properties, independent of the widgets GTK recycles.
use arut_product_session::MatchRange;
use gtk::{glib, pango};
mod imp {
    use gtk::{glib, pango, prelude::*, subclass::prelude::*};
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
        #[property(get, set)]
        preview: RefCell<String>,
        /// Where the current query matched the title, as Pango already wants
        /// it. A row carries the rendering, not the rule.
        #[property(get, set, nullable)]
        highlight: RefCell<Option<pango::AttrList>>,
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

impl ConversationItem {
    /// Turns the projection's character ranges into Pango's byte offsets. The
    /// ranges are Unicode scalars into `title`; Pango indexes UTF-8, so the
    /// walk below is the conversion and not a second matching rule.
    pub fn highlight_of(title: &str, ranges: &[MatchRange]) -> Option<pango::AttrList> {
        if ranges.is_empty() {
            return None;
        }
        let offsets: Vec<u32> = title
            .char_indices()
            .map(|(offset, _)| offset as u32)
            .chain(std::iter::once(title.len() as u32))
            .collect();
        let attributes = pango::AttrList::new();
        for range in ranges {
            let (Some(&start), Some(&end)) = (
                offsets.get(range.start as usize),
                offsets.get(range.end as usize),
            ) else {
                continue;
            };
            let mut weight = pango::AttrInt::new_weight(pango::Weight::Bold);
            weight.set_start_index(start);
            weight.set_end_index(end);
            attributes.insert(weight);
            let mut underline = pango::AttrInt::new_underline(pango::Underline::Single);
            underline.set_start_index(start);
            underline.set_end_index(end);
            attributes.insert(underline);
        }
        Some(attributes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_ranges_become_utf8_offsets() {
        let title = "Café société";
        let ranges = [MatchRange { start: 5, end: 12 }];
        let attributes = ConversationItem::highlight_of(title, &ranges).expect("a highlight");
        let bounds: Vec<(u32, u32)> = attributes
            .attributes()
            .iter()
            .map(|attribute| (attribute.start_index(), attribute.end_index()))
            .collect();
        // "société" starts after "Café " -- five characters, six bytes, and
        // runs to the end of a string that is longer in bytes than in chars.
        assert_eq!(bounds, [(6, 15), (6, 15)]);
        assert_eq!(&title[6..15], "société");
        assert!(ConversationItem::highlight_of(title, &[]).is_none());
    }
}
