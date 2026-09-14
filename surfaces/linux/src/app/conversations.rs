use crate::app::{conversation_model::ConversationItem, observe::Tasks, strings};
use arut_i18n::Message;
use arut_product_session::ProductSession;
use gtk::{gio, glib, prelude::*};
use relm4::{ComponentParts, ComponentSender, SimpleComponent};
use std::{collections::HashMap, rc::Rc};

pub struct Conversations {
    pub search: gtk::SearchEntry,
    _tasks: Tasks,
}

#[derive(Debug)]
pub enum Msg {
    /// Take keyboard focus, once the shell has revealed this sidebar.
    Focus,
}

#[relm4::component(pub)]
impl SimpleComponent for Conversations {
    type Init = Rc<ProductSession>;
    type Input = Msg;
    type Output = ();
    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 8,
            set_width_request: 260,
            add_css_class: "arut-sidebar",
            gtk::Label {
                set_label: &strings::show(&Message::LabelConversations),
                set_xalign: 0.0,
                add_css_class: "heading",
                set_margin_start: 8,
            },
            #[local_ref]
            search -> gtk::SearchEntry {},
            gtk::Overlay {
                set_vexpand: true,
                gtk::ScrolledWindow {
                    set_vexpand: true,
                    add_css_class: "arut-conversations",
                    set_hscrollbar_policy: gtk::PolicyType::Never,
                    update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::LabelConversations))],
                    #[local_ref]
                    list -> gtk::ListView {},
                },
                #[name = "empty_search"]
                add_overlay = &gtk::Label {
                    set_label: &strings::show(&Message::ConversationSearchEmpty),
                    set_wrap: true,
                    set_justify: gtk::Justification::Center,
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Start,
                    set_margin_top: 24,
                    set_margin_start: 16,
                    set_margin_end: 16,
                    set_can_target: false,
                    add_css_class: "dim-label",
                },
            },
        }
    }
    fn init(
        session: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let store = gio::ListStore::new::<ConversationItem>();
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some(&strings::show(
            &Message::ConversationSearchPlaceholder,
        )));
        search.update_property(&[gtk::accessible::Property::Label(&strings::show(
            &Message::ActionSearchConversations,
        ))]);
        let filter = gtk::StringFilter::builder()
            .expression(gtk::PropertyExpression::new(
                ConversationItem::static_type(),
                gtk::Expression::NONE,
                "title",
            ))
            .match_mode(gtk::StringFilterMatchMode::Substring)
            .ignore_case(true)
            .build();
        search
            .bind_property("text", &filter, "search")
            .transform_to(|_, text: String| Some(text.trim().to_owned()))
            .sync_create()
            .build();
        let selection = gtk::SingleSelection::new(Some(gtk::FilterListModel::new(
            Some(store.clone()),
            Some(filter.clone()),
        )));
        selection.set_autoselect(false);
        selection.set_can_unselect(true);
        // Filtering resets GTK's selected index. Re-apply the session's
        // selection from here, after the binding above has refiltered: doing it
        // from the model's own items-changed re-enters GtkSingleSelection.
        search.connect_notify_local(Some("text"), {
            let (selection, session) = (selection.clone(), session.clone());
            move |_, _| select_row(&selection, session.selected_id().as_deref())
        });
        let list = gtk::ListView::new(Some(selection.clone()), Some(row_factory()));
        list.add_css_class("navigation-sidebar");
        list.set_single_click_activate(true);
        list.update_property(&[gtk::accessible::Property::Label(&strings::show(
            &Message::LabelConversations,
        ))]);
        list.connect_activate({
            let (selection, session) = (selection.clone(), session.clone());
            move |_, position| {
                if let Some(row) = selection
                    .item(position)
                    .and_then(|object| object.downcast::<ConversationItem>().ok())
                {
                    session.select(Some(row.id()));
                }
            }
        });
        let mut model = Self {
            search: search.clone(),
            _tasks: Tasks::default(),
        };
        let widgets = view_output!();
        empty_search_expression(&selection, &search).bind(
            &widgets.empty_search,
            "visible",
            None::<&gtk::Widget>,
        );
        // One row object and one unread observer per conversation, keyed by id,
        // so neither accumulates as summaries are refreshed.
        let mut rows: HashMap<String, (ConversationItem, Tasks)> = HashMap::new();
        model
            ._tasks
            .observe(session.conversations_changes(), move || {
                let selected = session.selected_id();
                let summaries = session.chat_summaries();
                rows.retain(|id, _| summaries.iter().any(|summary| &summary.id == id));
                let mut titles_changed = false;
                let ordered: Vec<ConversationItem> = summaries
                    .iter()
                    .map(|summary| {
                        let (row, _) = rows.entry(summary.id.clone()).or_insert_with(|| {
                            let row = ConversationItem::default();
                            row.set_id(summary.id.as_str());
                            let tasks = follow_unread(&session, &summary.id, &row);
                            (row, tasks)
                        });
                        if row.title() != summary.title {
                            row.set_title(summary.title.as_str());
                            titles_changed = true;
                        }
                        if row.preview() != summary.preview {
                            row.set_preview(summary.preview.as_str());
                        }
                        if selected.as_ref() == Some(&summary.id) {
                            row.set_unread(false);
                        }
                        row.clone()
                    })
                    .collect();
                store.splice(0, store.n_items(), &ordered);
                // GTK filters watch items-changed, not item property notifies.
                if titles_changed {
                    filter.changed(gtk::FilterChange::Different);
                }
                select_row(&selection, selected.as_deref());
            });
        ComponentParts { model, widgets }
    }
    fn update(&mut self, _message: Msg, _sender: ComponentSender<Self>) {
        self.search.grab_focus();
    }
}

fn select_row(selection: &gtk::SingleSelection, selected: Option<&str>) {
    let position = (0..selection.n_items()).find(|&position| {
        selection
            .item(position)
            .and_then(|object| object.downcast::<ConversationItem>().ok())
            .is_some_and(|row| Some(row.id().as_str()) == selected)
    });
    selection.set_selected(position.unwrap_or(gtk::INVALID_LIST_POSITION));
}

/// Marks a row unread while a different conversation is showing. Keyed by
/// conversation id so the task goes when the conversation does.
fn follow_unread(session: &Rc<ProductSession>, id: &str, row: &ConversationItem) -> Tasks {
    let mut tasks = Tasks::default();
    if let Some(chat) = session.select_chat(id) {
        let (row, session, id) = (row.clone(), session.clone(), id.to_owned());
        let mut previous = chat.state().last_message_id;
        tasks.observe(chat.changes(), move || {
            let latest = chat.state().last_message_id;
            if latest > previous && session.selected_id().as_ref() != Some(&id) {
                row.set_unread(true);
            }
            previous = latest;
        });
    }
    tasks
}

/// Shown only when a search is active and matches nothing, evaluated by GTK
/// from the two properties it depends on.
fn empty_search_expression(
    selection: &gtk::SingleSelection,
    search: &gtk::SearchEntry,
) -> gtk::ClosureExpression {
    let matches = gtk::PropertyExpression::new(
        gtk::SingleSelection::static_type(),
        Some(gtk::ConstantExpression::new(selection)),
        "n-items",
    );
    let query = gtk::PropertyExpression::new(
        gtk::SearchEntry::static_type(),
        Some(gtk::ConstantExpression::new(search)),
        "text",
    );
    gtk::ClosureExpression::new::<bool>(
        [matches.upcast(), query.upcast()],
        glib::closure!(|_: Option<glib::Object>, matches: u32, query: &str| {
            matches == 0 && !query.trim().is_empty()
        }),
    )
}

fn row_factory() -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().unwrap();
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.add_css_class("arut-conversation-row");
        let labels = gtk::Box::new(gtk::Orientation::Vertical, 4);
        labels.set_hexpand(true);
        let title = gtk::Label::new(None);
        title.add_css_class("heading");
        title.set_xalign(0.0);
        title.set_hexpand(true);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let unread = gtk::Image::from_icon_name("mail-unread-symbolic");
        unread.update_property(&[gtk::accessible::Property::Label(&strings::show(
            &Message::LabelUnreadMessages,
        ))]);
        let preview = gtk::Label::new(None);
        preview.set_xalign(0.0);
        preview.set_ellipsize(gtk::pango::EllipsizeMode::End);
        preview.add_css_class("dim-label");
        labels.append(&title);
        labels.append(&preview);
        row.append(&labels);
        row.append(&unread);
        // Expressions follow ListItem.item on recycling; no signal handlers
        // or bindings accumulate when an existing row is rebound.
        let expression = gtk::PropertyExpression::new(
            gtk::ListItem::static_type(),
            None::<gtk::Expression>,
            "item",
        );
        expression
            .chain_property::<ConversationItem>("title")
            .bind(&title, "label", Some(item));
        expression
            .chain_property::<ConversationItem>("preview")
            .bind(&preview, "label", Some(item));
        expression
            .chain_property::<ConversationItem>("unread")
            .bind(&unread, "visible", Some(item));
        item.set_child(Some(&row));
    });
    factory
}
