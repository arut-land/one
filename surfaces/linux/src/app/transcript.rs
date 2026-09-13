use crate::app::{
    message_model::Messages,
    observe::{Tasks, ViewState},
    strings,
};
use arut_i18n::Message;
use arut_product_session::chat::{ChatClient, ChatMessage, ChatRole};
use gtk::{gio, glib, prelude::*};
use relm4::{ComponentParts, ComponentSender, SimpleComponent};
use std::{cell::Cell, rc::Rc};

pub struct Transcript {
    pub adjustment: gtk::Adjustment,
    pub restore_position: Rc<Cell<Option<f64>>>,
    _tasks: Tasks,
}

#[relm4::component(pub)]
impl SimpleComponent for Transcript {
    type Init = ChatClient;
    type Input = ();
    type Output = Option<String>;
    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_vexpand: true,
            set_hexpand: true,
            gtk::Overlay {
                set_vexpand: true,
            #[name = "scroll"]
            gtk::ScrolledWindow {
                set_vexpand: true,
                set_hscrollbar_policy: gtk::PolicyType::Never,
                update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::LabelTranscript))],
                #[local_ref]
                list -> gtk::ListView {},
            },
                #[name = "empty"]
                add_overlay = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 12,
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,
                    set_can_target: false,
                    gtk::Image {
                        set_icon_name: Some("mail-message-new-symbolic"),
                        set_pixel_size: 48,
                        set_accessible_role: gtk::AccessibleRole::Presentation,
                        add_css_class: "dim-label",
                    },
                    gtk::Label {
                        set_label: &strings::show(&Message::ChatEmptyTitle),
                        set_wrap: true,
                        add_css_class: "title-2",
                    },
                    gtk::Label {
                        set_label: &strings::show(&Message::ChatEmptyHint),
                        set_wrap: true,
                        set_max_width_chars: 36,
                        add_css_class: "dim-label",
                    },
                },
            },
            #[name = "latest"]
            gtk::Revealer {
                set_transition_type: gtk::RevealerTransitionType::SlideUp,
                gtk::Button {
                    set_halign: gtk::Align::Center,
                    set_icon_name: "go-bottom-symbolic",
                    set_action_name: Some("transcript.latest"),
                    set_tooltip_text: Some(&strings::show(&Message::ActionScrollToLatest)),
                    update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::ActionScrollToLatest))],
                },
            },
            #[name = "status"]
            gtk::Label {
                update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::LabelMessageStatus))],
            },
        }
    }
    fn init(
        chat: ChatClient,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let messages = Messages::default();
        let factory = message_factory();
        let list = gtk::ListView::new(
            Some(gtk::NoSelection::new(Some(messages.clone()))),
            Some(factory),
        );
        list.add_css_class("arut-transcript");
        list.set_accessible_role(gtk::AccessibleRole::Log);
        list.update_property(&[gtk::accessible::Property::Label(&strings::show(
            &Message::LabelTranscript,
        ))]);
        let mut model = Self {
            adjustment: gtk::Adjustment::default(),
            restore_position: Rc::default(),
            _tasks: Tasks::default(),
        };
        let widgets = view_output!();
        model.adjustment = widgets.scroll.vadjustment();
        let near_bottom = Rc::new(Cell::new(true));
        model.adjustment.connect_value_changed({
            let near = near_bottom.clone();
            let latest = widgets.latest.clone();
            move |a| {
                near.set(a.upper() - a.page_size() - a.value() < 48.0);
                latest.set_reveal_child(!near.get());
            }
        });
        model.adjustment.connect_changed({
            let near = near_bottom.clone();
            let restore = model.restore_position.clone();
            move |a| {
                if a.page_size() > 0.0
                    && let Some(position) = restore.take()
                {
                    near.set(false);
                    a.set_value(position);
                } else if near.get() {
                    a.set_value((a.upper() - a.page_size()).max(0.0));
                }
            }
        });
        let actions = gio::SimpleActionGroup::new();
        let latest = gio::SimpleAction::new("latest", None);
        latest.connect_activate({
            let a = model.adjustment.clone();
            move |_, _| a.set_value((a.upper() - a.page_size()).max(0.0))
        });
        actions.add_action(&latest);
        root.insert_action_group("transcript", Some(&actions));
        crate::app::theme::reveal_motion(&widgets.latest);
        let state = ViewState::default();
        state
            .bind_property("status", &widgets.status, "label")
            .sync_create()
            .build();
        state
            .bind_property("status", &widgets.status, "visible")
            .transform_to(|_, text: String| Some(!text.is_empty()))
            .sync_create()
            .build();
        let empty = widgets.empty.clone();
        let changes = chat.changes();
        let mut previous_id = None;
        model._tasks.observe(changes, move || {
            let snapshot = chat.state();
            state.set_status(strings::chat(snapshot.status, snapshot.error));
            if snapshot.id != previous_id {
                previous_id.clone_from(&snapshot.id);
                let _ = sender.output(snapshot.id);
            }
            let before = messages.n_items();
            messages.refresh(|key| chat.messages_after(key));
            empty.set_visible(messages.n_items() == 0);
            if before > 0 && messages.n_items() > before {
                // GTK's AT-SPI announcement is separate from row recycling.
                for index in before..messages.n_items() {
                    let row = messages
                        .item(index)
                        .unwrap()
                        .downcast::<glib::BoxedAnyObject>()
                        .unwrap();
                    list.announce(
                        &row.borrow::<ChatMessage>().text,
                        gtk::AccessibleAnnouncementPriority::Medium,
                    );
                }
            }
        });
        ComponentParts { model, widgets }
    }
}

fn message_factory() -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().unwrap();
        let container = gtk::Box::new(gtk::Orientation::Vertical, 8);
        container.set_margin_start(12);
        container.set_margin_end(12);
        container.set_margin_bottom(4);
        let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
        row.add_css_class("arut-message");
        let timestamp = gtk::Label::new(None);
        timestamp.add_css_class("dim-label");
        container.append(&timestamp);
        container.append(&row);
        let role = gtk::Label::new(None);
        role.set_xalign(0.0);
        role.add_css_class("heading");
        row.append(&role);
        let text = gtk::Label::new(None);
        text.set_xalign(0.0);
        text.set_wrap(true);
        text.set_max_width_chars(72);
        text.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        text.set_selectable(true);
        let menu = gio::Menu::new();
        menu.append(
            Some(&strings::show(&Message::ActionCopyMessage)),
            Some("message.copy"),
        );
        text.set_extra_menu(Some(&menu));
        let actions = gio::SimpleActionGroup::new();
        let copy = gio::SimpleAction::new("copy", None);
        let weak = text.downgrade();
        copy.connect_activate(move |_, _| {
            if let Some(text) = weak.upgrade() {
                text.clipboard().set_text(&text.text());
            }
        });
        actions.add_action(&copy);
        text.insert_action_group("message", Some(&actions));
        row.append(&text);
        item.set_child(Some(&container));
    });
    factory.connect_bind(|_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().unwrap();
        let object = item
            .item()
            .unwrap()
            .downcast::<glib::BoxedAnyObject>()
            .unwrap();
        let message = object.borrow::<ChatMessage>();
        let container = item.child().unwrap().downcast::<gtk::Box>().unwrap();
        let time = container
            .first_child()
            .unwrap()
            .downcast::<gtk::Label>()
            .unwrap();
        let row = time.next_sibling().unwrap().downcast::<gtk::Box>().unwrap();
        let role = row.first_child().unwrap().downcast::<gtk::Label>().unwrap();
        let text = role
            .next_sibling()
            .unwrap()
            .downcast::<gtk::Label>()
            .unwrap();
        let starts_time = message.starts_time_group;
        let starts_speaker = message.starts_speaker_group;
        let formatted = i64::try_from(message.accepted_at_ms / 1000)
            .ok()
            .and_then(|seconds| glib::DateTime::from_unix_local(seconds).ok())
            .and_then(|date| date.format("%b %e, %H:%M").ok())
            .unwrap_or_default();
        time.set_label(&formatted);
        time.set_visible(starts_time && message.accepted_at_ms > 0);
        role.set_label(&strings::role(message.role));
        role.set_visible(starts_speaker);
        text.set_label(&message.text);
        text.set_tooltip_text((message.accepted_at_ms > 0).then_some(formatted.as_str()));
        container.set_margin_top(if starts_speaker { 12 } else { 0 });
        row.set_halign(if message.role == ChatRole::User {
            gtk::Align::End
        } else {
            gtk::Align::Start
        });
        row.set_margin_start(if message.role == ChatRole::User {
            48
        } else {
            0
        });
        row.set_margin_end(if message.role == ChatRole::User {
            0
        } else {
            48
        });
        if message.role == ChatRole::User {
            row.add_css_class("arut-outgoing");
        } else {
            row.remove_css_class("arut-outgoing");
        }
    });
    factory
}
