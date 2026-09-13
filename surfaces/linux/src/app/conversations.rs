use crate::app::{conversation_model::ConversationItem, observe::Tasks, strings};
use arut_i18n::Message;
use arut_product_session::ProductSession;
use gtk::{gio, prelude::*};
use relm4::{ComponentParts, ComponentSender, SimpleComponent};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub struct Conversations {
    pub search: gtk::SearchEntry,
    selected: Rc<RefCell<Option<String>>>,
    rows: Rc<RefCell<HashMap<String, ConversationItem>>>,
    selection: gtk::SingleSelection,
    _tasks: Tasks,
}
#[derive(Debug, Clone)]
pub enum Msg {
    Selected(Option<String>),
}

#[relm4::component(pub)]
impl SimpleComponent for Conversations {
    type Init = Rc<ProductSession>;
    type Input = Msg;
    type Output = String;
    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 8,
            set_width_request: 240,
            set_margin_start: 8,
            set_margin_end: 8,
            #[local_ref]
            search -> gtk::SearchEntry {},
            gtk::ScrolledWindow {
                set_vexpand: true,
                add_css_class: "arut-conversations",
                set_hscrollbar_policy: gtk::PolicyType::Never,
                update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::LabelConversations))],
                #[local_ref]
                list -> gtk::ListView {},
            },
        }
    }
    fn init(
        session: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let store = gio::ListStore::new::<ConversationItem>();
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some(&strings::show(
            &Message::ConversationSearchPlaceholder,
        )));
        search.update_property(&[gtk::accessible::Property::Label(&strings::show(
            &Message::ActionSearchConversations,
        ))]);
        let query = Rc::new(RefCell::new(String::new()));
        let filter = gtk::CustomFilter::new({
            let query = query.clone();
            move |object| {
                object
                    .downcast_ref::<ConversationItem>()
                    .unwrap()
                    .title()
                    .to_lowercase()
                    .contains(query.borrow().as_str())
            }
        });
        let filtered = gtk::FilterListModel::new(Some(store.clone()), Some(filter.clone()));
        search.connect_search_changed(move |search| {
            *query.borrow_mut() = search.text().trim().to_lowercase();
            filter.changed(gtk::FilterChange::Different);
        });
        let selection = gtk::SingleSelection::new(Some(filtered));
        selection.set_autoselect(false);
        selection.set_can_unselect(true);
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let title = gtk::Label::new(None);
            title.set_xalign(0.0);
            title.set_hexpand(true);
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            let unread = gtk::Image::from_icon_name("mail-unread-symbolic");
            unread.update_property(&[gtk::accessible::Property::Label(&strings::show(
                &Message::LabelUnreadMessages,
            ))]);
            row.append(&title);
            row.append(&unread);
            // Expressions follow ListItem.item on recycling; no signal handlers
            // or bindings accumulate when an existing row is rebound.
            let expression = gtk::PropertyExpression::new(
                gtk::ListItem::static_type(),
                None::<gtk::Expression>,
                "item",
            );
            expression.chain_property::<ConversationItem>("title").bind(
                &title,
                "label",
                Some(item),
            );
            expression
                .chain_property::<ConversationItem>("unread")
                .bind(&unread, "visible", Some(item));
            item.set_child(Some(&row));
        });
        let list = gtk::ListView::new(Some(selection.clone()), Some(factory));
        list.add_css_class("navigation-sidebar");
        list.set_single_click_activate(true);
        list.update_property(&[gtk::accessible::Property::Label(&strings::show(
            &Message::LabelConversations,
        ))]);
        list.connect_activate({
            let selection = selection.clone();
            move |_, position| {
                if let Some(row) = selection
                    .item(position)
                    .and_then(|o| o.downcast::<ConversationItem>().ok())
                {
                    let _ = sender.output(row.id());
                }
            }
        });
        let mut model = Self {
            search: search.clone(),
            selected: Rc::default(),
            rows: Rc::default(),
            selection,
            _tasks: Tasks::default(),
        };
        let widgets = view_output!();
        let (rows, selected) = (model.rows.clone(), model.selected.clone());
        let mut row_tasks = Tasks::default();
        model
            ._tasks
            .observe(session.conversations_changes(), move || {
                for (position, summary) in session.chat_summaries().into_iter().enumerate() {
                    let row = rows
                        .borrow_mut()
                        .entry(summary.id.clone())
                        .or_insert_with(|| {
                            let row = ConversationItem::default();
                            row.set_id(summary.id.as_str());
                            if let Some(chat) = session.select_chat(&summary.id) {
                                let (row, selected, id) =
                                    (row.clone(), selected.clone(), summary.id.clone());
                                let mut previous = chat.state().last_message_id;
                                row_tasks.observe(chat.changes(), move || {
                                    let latest = chat.state().last_message_id;
                                    if latest > previous && selected.borrow().as_ref() != Some(&id)
                                    {
                                        row.set_unread(true);
                                    }
                                    previous = latest;
                                });
                            }
                            row
                        })
                        .clone();
                    row.set_title(summary.title.as_str());
                    let position = position as u32;
                    if store.item(position).as_ref() != Some(row.upcast_ref()) {
                        if let Some(old) = store.find(&row) {
                            store.remove(old);
                        }
                        store.insert(position, &row);
                    }
                }
            });
        ComponentParts { model, widgets }
    }
    fn update(&mut self, Msg::Selected(selected): Msg, _: ComponentSender<Self>) {
        *self.selected.borrow_mut() = selected.clone();
        if let Some(row) = selected
            .as_ref()
            .and_then(|id| self.rows.borrow().get(id).cloned())
        {
            row.set_unread(false);
        }
        let position = (0..self.selection.n_items()).find(|&p| {
            self.selection
                .item(p)
                .and_then(|o| o.downcast::<ConversationItem>().ok())
                .is_some_and(|row| Some(row.id()) == selected)
        });
        self.selection
            .set_selected(position.unwrap_or(gtk::INVALID_LIST_POSITION));
    }
}
