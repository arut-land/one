use crate::app::{motion, strings};
use crate::glib_observe::{Rows, Tasks};
use arut_i18n::Message;
use arut_product_session::chat::{ChatClient, ChatMessage, ChatRole};
use gtk::{gio, glib, prelude::*};
use relm4::{ComponentParts, ComponentSender, SimpleComponent};
use std::{cell::Cell, rc::Rc};

/// How long after a refresh its appended rows still count as arriving, which
/// is what gets them the arrival animation and not a scrolled-into-view row.
const FRESH_US: i64 = 1_000_000;

pub struct Transcript {
    chat: ChatClient,
    messages: Rows,
    list: gtk::ListView,
    status: gtk::Label,
    empty: gtk::Box,
    adjustment: gtk::Adjustment,
    restore: Rc<Cell<Option<f64>>>,
    near_bottom: Rc<Cell<bool>>,
    /// The first key the latest refresh appended and when it landed. Rows at
    /// or past that key, bound within `FRESH_US`, animate in.
    fresh: Rc<Cell<(u64, i64)>>,
    motion: Option<motion::Motion>,
    tasks: Tasks,
}

pub enum Msg {
    /// Show another conversation in the same widgets.
    Chat(Box<ChatClient>),
    /// Reading position to return to once this conversation has laid out.
    Restore(f64),
    Latest,
}

impl std::fmt::Debug for Msg {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Chat(chat) => formatter.debug_tuple("Chat").field(&chat.id()).finish(),
            Self::Restore(position) => formatter.debug_tuple("Restore").field(position).finish(),
            Self::Latest => formatter.write_str("Latest"),
        }
    }
}

/// The reading position of the conversation being left, so the shell can hand
/// it back the next time that conversation is shown.
#[derive(Debug)]
pub struct Position(pub Option<String>, pub f64);

#[relm4::component(pub)]
impl SimpleComponent for Transcript {
    type Init = ChatClient;
    type Input = Msg;
    type Output = Position;
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
                    set_spacing: 6,
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,
                    set_can_target: false,
                    set_margin_start: 24,
                    set_margin_end: 24,
                    gtk::Image {
                        set_icon_name: Some("arut-empty-symbolic"),
                        set_pixel_size: 48,
                        set_halign: gtk::Align::Center,
                        set_margin_bottom: 12,
                        set_accessible_role: gtk::AccessibleRole::Presentation,
                        add_css_class: "arut-empty-icon",
                    },
                    gtk::Label {
                        set_label: &strings::show(&Message::ChatEmptyTitle),
                        set_wrap: true,
                        set_justify: gtk::Justification::Center,
                        add_css_class: "arut-empty-title",
                    },
                    gtk::Label {
                        set_label: &strings::show(&Message::ChatEmptyHint),
                        set_wrap: true,
                        set_justify: gtk::Justification::Center,
                        set_max_width_chars: 36,
                        add_css_class: "arut-empty-hint",
                    },
                },
                // Centred above the composer, where it covers neither an
                // outgoing bubble on the right nor the scrollbar.
                #[name = "latest"]
                add_overlay = &gtk::Revealer {
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::End,
                    set_margin_bottom: 12,
                    set_transition_type: gtk::RevealerTransitionType::SlideUp,
                    set_transition_duration: 200,
                    gtk::Button {
                        add_css_class: "circular",
                        add_css_class: "arut-latest",
                        set_icon_name: "arut-latest-symbolic",
                        set_action_name: Some("win.latest"),
                        set_tooltip_text: Some(&strings::show(&Message::ActionScrollToLatest)),
                        update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::ActionScrollToLatest))],
                    },
                },
            },
            #[name = "status"]
            gtk::Label {
                add_css_class: "arut-caption",
                // A caption that appears and changes must be announced, which
                // is what the status role is for.
                set_accessible_role: gtk::AccessibleRole::Status,
                update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::LabelMessageStatus))],
            },
        }
    }
    fn init(
        chat: ChatClient,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let messages = Rows::new::<ChatMessage>(|message| message.id);
        let fresh = Rc::new(Cell::new((u64::MAX, 0)));
        let list = gtk::ListView::new(
            Some(gtk::NoSelection::new(Some(messages.clone()))),
            Some(message_factory(fresh.clone())),
        );
        list.add_css_class("arut-transcript");
        list.set_accessible_role(gtk::AccessibleRole::Log);
        list.update_property(&[gtk::accessible::Property::Label(&strings::show(
            &Message::LabelTranscript,
        ))]);
        let mut model = Self {
            chat: chat.clone(),
            messages,
            list: list.clone(),
            status: gtk::Label::new(None),
            empty: gtk::Box::default(),
            adjustment: gtk::Adjustment::default(),
            restore: Rc::default(),
            near_bottom: Rc::new(Cell::new(true)),
            fresh,
            motion: None,
            tasks: Tasks::default(),
        };
        let widgets = view_output!();
        model.status = widgets.status.clone();
        model.empty = widgets.empty.clone();
        model.adjustment = widgets.scroll.vadjustment();
        model.adjustment.connect_value_changed({
            let (near, latest) = (model.near_bottom.clone(), widgets.latest.clone());
            move |adjustment| {
                near.set(adjustment.upper() - adjustment.page_size() - adjustment.value() < 48.0);
                latest.set_reveal_child(!near.get());
            }
        });
        model.adjustment.connect_changed({
            let (near, restore) = (model.near_bottom.clone(), model.restore.clone());
            move |adjustment| {
                if adjustment.page_size() > 0.0
                    && let Some(position) = restore.take()
                {
                    near.set(false);
                    adjustment.set_value(position);
                } else if near.get() {
                    adjustment.set_value((adjustment.upper() - adjustment.page_size()).max(0.0));
                }
            }
        });
        model.follow(chat);
        ComponentParts { model, widgets }
    }
    fn update(&mut self, message: Msg, sender: ComponentSender<Self>) {
        match message {
            Msg::Chat(chat) => {
                let _ = sender.output(Position(self.chat.id(), self.adjustment.value()));
                self.motion = None;
                self.messages.reset();
                self.near_bottom.set(true);
                self.chat = (*chat).clone();
                self.follow(*chat);
            }
            Msg::Restore(position) => self.restore.set(Some(position)),
            Msg::Latest => {
                // A deliberate jump eases; automatic tail-following in the
                // `changed` handler above stays immediate.
                let adjustment = self.adjustment.clone();
                let from = adjustment.value();
                let target = (adjustment.upper() - adjustment.page_size()).max(0.0);
                self.motion = motion::run(&self.list, motion::DURATION_MS, move |progress| {
                    adjustment.set_value(from + (target - from) * progress);
                });
            }
        }
    }
}

impl Transcript {
    /// Replaces the conversation watch. Dropping the previous `Tasks` cancels
    /// it before the first refresh of the new one reaches these widgets.
    fn follow(&mut self, chat: ChatClient) {
        let mut tasks = Tasks::default();
        let (messages, list, status, empty, fresh) = (
            self.messages.clone(),
            self.list.clone(),
            self.status.clone(),
            self.empty.clone(),
            self.fresh.clone(),
        );
        tasks.observe(chat.changes(), move || {
            let snapshot = chat.state();
            let caption = strings::chat(snapshot.status, snapshot.error);
            status.set_visible(!caption.is_empty());
            status.set_label(&caption);
            empty.set_visible(snapshot.is_empty);
            let before = messages.n_items();
            messages.refresh(snapshot.last_message_id, |key| chat.messages_after(key));
            if before > 0 && messages.n_items() > before {
                // Rows appended to a transcript already on screen arrive; the
                // first fill of a conversation is a page, not an arrival.
                let first = messages
                    .item(before)
                    .unwrap()
                    .downcast::<glib::BoxedAnyObject>()
                    .unwrap();
                fresh.set((first.borrow::<ChatMessage>().id, glib::monotonic_time()));
                // GTK's AT-SPI announcement is separate from row recycling. The
                // person's own message was just typed here, so announcing it
                // back reads their own words to them twice.
                for index in before..messages.n_items() {
                    let row = messages
                        .item(index)
                        .unwrap()
                        .downcast::<glib::BoxedAnyObject>()
                        .unwrap();
                    let message = row.borrow::<ChatMessage>();
                    if message.role == ChatRole::User {
                        continue;
                    }
                    list.announce(&message.text, gtk::AccessibleAnnouncementPriority::Medium);
                }
            }
        });
        self.tasks = tasks;
    }
}

fn set_class(widget: &impl IsA<gtk::Widget>, class: &str, active: bool) {
    if active {
        widget.add_css_class(class);
    } else {
        widget.remove_css_class(class);
    }
}

/// Starts the arrival keyframes on a row, or clears them. A recycled widget
/// may still carry the class from the row it showed before, and re-adding the
/// same class in the same frame is no style change to GTK, so the class name
/// alternates: what changes is what animates.
fn arrive(widget: &impl IsA<gtk::Widget>, fresh: bool) {
    let was_a = widget.has_css_class("arut-new-a");
    widget.remove_css_class("arut-new-a");
    widget.remove_css_class("arut-new-b");
    if fresh {
        widget.add_css_class(if was_a { "arut-new-b" } else { "arut-new-a" });
    }
}

/// The locale's short clock. `%X` carries seconds, which a transcript has no
/// use for; whether the locale wants a twelve-hour clock is read off `%p`,
/// which an hour-cycle-24 locale leaves empty.
fn clock(stamp: &glib::DateTime) -> String {
    let twelve_hour = stamp.format("%p").is_ok_and(|marker| !marker.is_empty());
    stamp
        .format(if twelve_hour { "%l:%M %p" } else { "%H:%M" })
        .map(|formatted| formatted.trim().to_owned())
        .unwrap_or_default()
}

/// The date a group header shows: relative while it is recent, the weekday
/// within the week, the month and day within the year, and the locale's full
/// date past that. GLib formats the names, so they follow the person's locale.
fn day(stamp: &glib::DateTime) -> String {
    let format = |pattern: &str| {
        stamp
            .format(pattern)
            .map(|formatted| formatted.trim().to_owned())
            .unwrap_or_default()
    };
    let Ok(now) = glib::DateTime::now_local() else {
        return format("%x");
    };
    if stamp.ymd() == now.ymd() {
        return strings::show(&Message::TranscriptToday);
    }
    if now
        .add_days(-1)
        .is_ok_and(|yesterday| yesterday.ymd() == stamp.ymd())
    {
        return strings::show(&Message::TranscriptYesterday);
    }
    let elapsed_days = (now.to_unix() - stamp.to_unix()) / 86_400;
    if (0..7).contains(&elapsed_days) {
        return format("%A");
    }
    if stamp.year() == now.year() {
        return format("%B %-d");
    }
    format("%x")
}

fn message_factory(fresh: Rc<Cell<(u64, i64)>>) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().unwrap();
        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.set_margin_start(16);
        container.set_margin_end(16);
        let row = gtk::Box::new(gtk::Orientation::Vertical, 2);
        row.add_css_class("arut-message");
        let date = gtk::Label::new(None);
        date.set_halign(gtk::Align::Center);
        date.set_margin_top(12);
        date.set_margin_bottom(12);
        date.add_css_class("arut-date");
        container.append(&date);
        container.append(&row);
        let role = gtk::Label::new(None);
        role.set_xalign(0.0);
        role.add_css_class("arut-role");
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
        // The time is readable without a pointer: a tooltip is invisible to
        // touch and to a screen reader walking the row.
        let clock = gtk::Label::new(None);
        clock.set_xalign(1.0);
        clock.add_css_class("arut-message-time");
        row.append(&clock);
        item.set_child(Some(&container));
    });
    factory.connect_unbind(|_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().unwrap();
        if let Some(container) = item.child() {
            arrive(&container, false);
        }
    });
    factory.connect_bind(move |_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().unwrap();
        let object = item
            .item()
            .unwrap()
            .downcast::<glib::BoxedAnyObject>()
            .unwrap();
        let message = object.borrow::<ChatMessage>();
        let container = item.child().unwrap().downcast::<gtk::Box>().unwrap();
        let date = container
            .first_child()
            .unwrap()
            .downcast::<gtk::Label>()
            .unwrap();
        let row = date.next_sibling().unwrap().downcast::<gtk::Box>().unwrap();
        let role = row.first_child().unwrap().downcast::<gtk::Label>().unwrap();
        let text = role
            .next_sibling()
            .unwrap()
            .downcast::<gtk::Label>()
            .unwrap();
        let clock_label = text
            .next_sibling()
            .unwrap()
            .downcast::<gtk::Label>()
            .unwrap();
        // GLib's own locale formats, so the hour cycle, the field order and
        // the month names come from the person's settings. The group header
        // carries the date and the row caption the time, so neither repeats
        // the other; the tooltip carries the full stamp.
        let stamp = i64::try_from(message.accepted_at_ms / 1000)
            .ok()
            .filter(|_| message.accepted_at_ms > 0)
            .and_then(|seconds| glib::DateTime::from_unix_local(seconds).ok());
        let (header, minute, full) = stamp.as_ref().map_or_else(
            || (String::new(), String::new(), String::new()),
            |stamp| {
                (
                    day(stamp),
                    clock(stamp),
                    stamp
                        .format("%x %X")
                        .map(|formatted| formatted.to_string())
                        .unwrap_or_default(),
                )
            },
        );
        date.set_label(&header);
        date.set_visible(message.starts_time_group && stamp.is_some());
        let outgoing = message.role == ChatRole::User;
        // Alignment and colour already say who sent an outgoing message; the
        // name is for the other side, once per burst.
        role.set_label(&strings::role(message.role));
        role.set_visible(message.starts_speaker_group && !outgoing);
        text.set_label(&message.text);
        text.set_tooltip_text((!full.is_empty()).then_some(full.as_str()));
        // One caption per speaker group, on the row that ends it: a time beside
        // every line of the same burst is noise.
        clock_label.set_label(&minute);
        clock_label.set_visible(stamp.is_some() && message.ends_speaker_group);
        // Rust decides where a speaker group ends, so no row reads the one
        // behind or ahead of it to space itself (B.6).
        container.set_margin_bottom(if message.ends_speaker_group { 14 } else { 2 });
        row.set_halign(if outgoing {
            gtk::Align::End
        } else {
            gtk::Align::Start
        });
        row.set_margin_start(if outgoing { 64 } else { 0 });
        row.set_margin_end(if outgoing { 0 } else { 64 });
        set_class(&row, "arut-outgoing", outgoing);
        set_class(&row, "arut-incoming", !outgoing);
        set_class(&row, "arut-group-start", message.starts_speaker_group);
        set_class(&row, "arut-group-end", message.ends_speaker_group);
        let (first, landed) = fresh.get();
        arrive(
            &container,
            message.id >= first && glib::monotonic_time() - landed < FRESH_US,
        );
    });
    factory
}
