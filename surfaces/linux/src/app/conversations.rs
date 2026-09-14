use crate::app::{Session, conversation_model::ConversationItem, strings};
use crate::glib_observe::Tasks;
use arut_i18n::Message;
use gtk::{gio, prelude::*};
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
    type Init = Rc<Session>;
    type Input = Msg;
    type Output = ();
    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 8,
            set_width_request: 220,
            add_css_class: "arut-sidebar",
            #[local_ref]
            search -> gtk::SearchEntry {
                set_margin_bottom: 4,
            },
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
                #[name = "empty"]
                add_overlay = &gtk::Label {
                    set_wrap: true,
                    set_visible: false,
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
        // One query, owned by the session: the matching rule, its casing and
        // the highlight offsets are all Rust's (ADR 0021).
        search.connect_notify_local(Some("text"), {
            let session = session.clone();
            move |search: &gtk::SearchEntry, _| {
                session.set_query(search.text().trim().to_owned());
            }
        });
        let selection = gtk::SingleSelection::new(Some(store.clone()));
        selection.set_autoselect(false);
        selection.set_can_unselect(true);
        let list = gtk::ListView::new(Some(selection.clone()), Some(row_factory(session.clone())));
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
        let empty = widgets.empty.clone();
        // One row object per conversation, keyed by id, so GTK rebinds an
        // existing widget instead of rebuilding it when a summary changes.
        let mut rows: HashMap<String, ConversationItem> = HashMap::new();
        model
            ._tasks
            .observe(session.conversations_changes(), move || {
                let selected = session.selected_id();
                let summaries = session.chat_summaries();
                // Only the unnarrowed list says which conversations still
                // exist; a query hides rows without ending them, and dropping
                // them here would rebuild the widget and lose the selection
                // when the query is cleared.
                if session.query().is_empty() {
                    rows.retain(|id, _| summaries.iter().any(|summary| &summary.id == id));
                }
                let ordered: Vec<ConversationItem> = summaries
                    .iter()
                    .map(|summary| {
                        let row = rows.entry(summary.id.clone()).or_insert_with(|| {
                            let row = ConversationItem::default();
                            row.set_id(summary.id.as_str());
                            row
                        });
                        row.set_title(summary.title.as_str());
                        row.set_preview(summary.preview.as_str());
                        row.set_unread(summary.unread);
                        row.set_highlight(ConversationItem::highlight_of(
                            &summary.title,
                            &summary.match_ranges,
                        ));
                        row.clone()
                    })
                    .collect();
                splice_changed(&store, &ordered);
                empty.set_visible(ordered.is_empty());
                empty.set_label(&strings::show(if session.query().is_empty() {
                    &Message::ChatHistoryEmpty
                } else {
                    &Message::ConversationSearchEmpty
                }));
                select_row(&selection, selected.as_deref());
            });
        ComponentParts { model, widgets }
    }
    fn update(&mut self, _message: Msg, _sender: ComponentSender<Self>) {
        self.search.grab_focus();
    }
}

/// Replaces only the range that actually moved. Row objects are keyed by
/// conversation id, so a shared prefix and suffix are the same objects and GTK
/// keeps their widgets, their scroll position and their selection.
fn splice_changed(store: &gio::ListStore, ordered: &[ConversationItem]) {
    let existing: Vec<ConversationItem> = store
        .iter::<ConversationItem>()
        .map(|row| row.expect("conversation row"))
        .collect();
    let same = |left: &ConversationItem, right: &ConversationItem| left == right;
    let head = existing
        .iter()
        .zip(ordered)
        .take_while(|(left, right)| same(left, right))
        .count();
    let tail = existing[head..]
        .iter()
        .rev()
        .zip(ordered[head..].iter().rev())
        .take_while(|(left, right)| same(left, right))
        .count();
    if head == existing.len() && head == ordered.len() {
        return;
    }
    let removed = (existing.len() - head - tail) as u32;
    store.splice(head as u32, removed, &ordered[head..ordered.len() - tail]);
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

fn row_factory(session: Rc<Session>) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().unwrap();
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.add_css_class("arut-conversation-row");
        let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        labels.set_hexpand(true);
        let title = gtk::Label::new(None);
        title.add_css_class("heading");
        title.set_xalign(0.0);
        title.set_hexpand(true);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        // An accent dot, the way every desktop marks unread now; the label
        // says what it means to a screen reader.
        let unread = gtk::Image::from_icon_name("arut-unread-symbolic");
        unread.add_css_class("arut-unread");
        unread.set_valign(gtk::Align::Center);
        unread.update_property(&[gtk::accessible::Property::Label(&strings::show(
            &Message::LabelUnreadMessages,
        ))]);
        let preview = gtk::Label::new(None);
        preview.set_xalign(0.0);
        preview.set_ellipsize(gtk::pango::EllipsizeMode::End);
        preview.add_css_class("arut-preview");
        labels.append(&title);
        labels.append(&preview);
        row.append(&labels);
        row.append(&unread);
        let menu = gio::Menu::new();
        menu.append(
            Some(&strings::show(&Message::ActionRenameConversation)),
            Some("conversation.rename"),
        );
        menu.append(
            Some(&strings::show(&Message::ActionDeleteConversation)),
            Some("conversation.delete"),
        );
        let menu_button = gtk::MenuButton::builder()
            .icon_name("arut-menu-symbolic")
            .menu_model(&menu)
            .valign(gtk::Align::Center)
            .build();
        menu_button.add_css_class("flat");
        menu_button.update_property(&[gtk::accessible::Property::Label(&strings::show(
            &Message::ActionMainMenu,
        ))]);
        row.append(&menu_button);

        let actions = gio::SimpleActionGroup::new();
        let rename = gio::SimpleAction::new("rename", None);
        rename.connect_activate({
            let (item, row, session) = (item.downgrade(), row.downgrade(), session.clone());
            move |_, _| {
                let (Some(item), Some(row)) = (item.upgrade(), row.upgrade()) else {
                    return;
                };
                let Some(conversation) = item
                    .item()
                    .and_then(|item| item.downcast::<ConversationItem>().ok())
                else {
                    return;
                };
                show_rename_dialog(&row, conversation, session.clone());
            }
        });
        actions.add_action(&rename);
        let delete = gio::SimpleAction::new("delete", None);
        delete.connect_activate({
            let (item, row, session) = (item.downgrade(), row.downgrade(), session.clone());
            move |_, _| {
                let (Some(item), Some(row)) = (item.upgrade(), row.upgrade()) else {
                    return;
                };
                let Some(conversation) = item
                    .item()
                    .and_then(|item| item.downcast::<ConversationItem>().ok())
                else {
                    return;
                };
                confirm_delete(&row, conversation.id(), session.clone());
            }
        });
        actions.add_action(&delete);
        row.insert_action_group("conversation", Some(&actions));

        let open_menu = gio::SimpleAction::new("menu", None);
        open_menu.connect_activate({
            let menu_button = menu_button.downgrade();
            move |_, _| {
                if let Some(popover) = menu_button.upgrade().and_then(|button| button.popover()) {
                    popover.popup();
                }
            }
        });
        actions.add_action(&open_menu);
        let shortcuts = gtk::ShortcutController::new();
        for trigger in ["Menu", "<Shift>F10"] {
            shortcuts.add_shortcut(gtk::Shortcut::new(
                gtk::ShortcutTrigger::parse_string(trigger),
                Some(gtk::NamedAction::new("conversation.menu")),
            ));
        }
        row.add_controller(shortcuts);
        let secondary_click = gtk::GestureClick::new();
        secondary_click.set_button(gtk::gdk::BUTTON_SECONDARY);
        secondary_click.connect_pressed({
            let menu_button = menu_button.downgrade();
            move |_, _, _, _| {
                if let Some(popover) = menu_button.upgrade().and_then(|button| button.popover()) {
                    popover.popup();
                }
            }
        });
        row.add_controller(secondary_click);
        // Expressions follow ListItem.item on recycling; no signal handlers
        // or bindings accumulate when an existing row is rebound.
        let expression = gtk::PropertyExpression::new(
            gtk::ListItem::static_type(),
            None::<gtk::Expression>,
            "item",
        );
        for (property, target, target_property) in [
            ("title", title.clone().upcast::<gtk::Widget>(), "label"),
            ("highlight", title.clone().upcast(), "attributes"),
            ("preview", preview.upcast(), "label"),
            ("unread", unread.upcast(), "visible"),
        ] {
            expression
                .chain_property::<ConversationItem>(property)
                .bind(&target, target_property, Some(item));
        }
        item.set_child(Some(&row));
    });
    factory
}

#[expect(
    deprecated,
    reason = "plain GTK has no non-deprecated dialog with an entry"
)]
fn show_rename_dialog(row: &gtk::Box, conversation: ConversationItem, session: Rc<Session>) {
    let Some(window) = row
        .root()
        .and_then(|root| root.downcast::<gtk::Window>().ok())
    else {
        return;
    };
    let dialog = gtk::Dialog::with_buttons(
        Some(&strings::show(&Message::ConversationRenameTitle)),
        Some(&window),
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[
            (
                &strings::show(&Message::ActionCancel),
                gtk::ResponseType::Cancel,
            ),
            (
                &strings::show(&Message::ActionSave),
                gtk::ResponseType::Accept,
            ),
        ],
    );
    dialog.set_default_response(gtk::ResponseType::Accept);
    let entry = gtk::Entry::new();
    entry.set_text(&conversation.title());
    entry.set_activates_default(true);
    entry.set_hexpand(true);
    entry.set_margin_top(12);
    entry.set_margin_bottom(12);
    entry.set_margin_start(12);
    entry.set_margin_end(12);
    entry.update_property(&[gtk::accessible::Property::Label(&strings::show(
        &Message::ConversationRenameTitle,
    ))]);
    dialog.content_area().append(&entry);
    if let Some(save) = dialog.widget_for_response(gtk::ResponseType::Accept) {
        save.add_css_class("suggested-action");
        save.set_sensitive(!entry.text().trim().is_empty());
        entry.connect_changed(move |entry| save.set_sensitive(!entry.text().trim().is_empty()));
    }
    dialog.connect_response(move |dialog, response| {
        if response == gtk::ResponseType::Accept {
            let title = entry.text().trim().to_owned();
            if !title.is_empty() {
                let (id, session) = (conversation.id(), session.clone());
                gtk::glib::spawn_future_local(async move {
                    session.rename(id, title).await;
                });
            }
        }
        dialog.close();
    });
    dialog.present();
}

fn confirm_delete(row: &gtk::Box, id: String, session: Rc<Session>) {
    let Some(window) = row
        .root()
        .and_then(|root| root.downcast::<gtk::Window>().ok())
    else {
        return;
    };
    let dialog = gtk::AlertDialog::builder()
        .modal(true)
        .message(strings::show(&Message::ConversationDeleteTitle))
        .detail(strings::show(&Message::ConversationDeleteMessage))
        .buttons([
            strings::show(&Message::ActionCancel),
            strings::show(&Message::ActionDeleteConversation),
        ])
        .cancel_button(0)
        .default_button(0)
        .build();
    gtk::glib::spawn_future_local(async move {
        if dialog.choose_future(Some(&window)).await == Ok(1) {
            session.delete(id).await;
        }
    });
}
