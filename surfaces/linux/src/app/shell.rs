use crate::app::{
    composer::{Composer, Msg as ComposerMsg},
    conversations::{Conversations, Msg as ConversationsMsg},
    navigation::Navigation,
    observe::Tasks,
    strings,
    transcript::{Msg as TranscriptMsg, Position, Transcript},
};
use arut_i18n::Message;
use arut_product_session::chat::ChatClient;
use arut_product_session::{FeatureAvailability, ProductSession};
use gtk::{gio, prelude::*};
use relm4::{Component, ComponentController, ComponentParts, ComponentSender, Controller};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub struct Shell {
    session: Rc<ProductSession>,
    navigation: Navigation,
    conversations: Controller<Conversations>,
    transcript: Controller<Transcript>,
    composer: Controller<Composer>,
    body: gtk::Box,
    drafts: HashMap<Option<String>, Controller<Composer>>,
    positions: HashMap<Option<String>, f64>,
    /// The conversation whose widgets are mounted. Shared with the watch that
    /// notices a pending conversation becoming an established one.
    displayed: Rc<RefCell<Option<String>>>,
    narrow: bool,
    title: String,
    availability: String,
    available: bool,
    restore_pending: bool,
    error: String,
    _tasks: Tasks,
    _chat_tasks: Tasks,
    _theme: crate::app::theme::Theme,
    _decorations: Option<crate::app::decorations::Decorations>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    New,
    ToggleSidebar,
    FocusComposer,
    Search,
    Escape,
    Narrow(bool),
    Conversations,
    Position(Option<String>, f64),
    Availability,
    Initialized(Result<(), arut_product_session::SessionError>),
}

#[relm4::component(pub)]
impl Component for Shell {
    type Init = Rc<ProductSession>;
    type Input = Msg;
    type Output = ();
    type CommandOutput = ();
    view! {
        #[name = "window"]
        gtk::ApplicationWindow {
            set_title: Some(&strings::show(&Message::AppName)),
            add_css_class: "arut-window",
            set_default_size: (900, 600),
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                #[name = "header"]
                gtk::HeaderBar {
                    #[name = "toolbar"]
                    pack_start = &gtk::Box {
                        set_spacing: 8,
                        add_css_class: "arut-toolbar",
                        #[name = "sidebar_toggle"]
                        gtk::ToggleButton {
                            set_icon_name: "sidebar-show-symbolic",
                            add_css_class: "flat",
                            #[watch]
                            set_active: !model.navigation.collapsed,
                            set_tooltip_text: Some(&strings::show(&Message::ActionToggleSidebarShortcut { shortcut: "Ctrl+B".into() })),
                            update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::ActionToggleSidebar))],
                        },
                        gtk::Button {
                            set_icon_name: "chat-message-new-symbolic",
                            add_css_class: "flat",
                            set_tooltip_text: Some(&strings::show(&Message::ActionNewConversationShortcut { shortcut: "Ctrl+N".into() })),
                            update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::ActionNewConversation))],
                            set_action_name: Some("win.new"),
                        },
                        gtk::Label {
                            set_hexpand: true,
                            set_xalign: 0.0,
                            set_ellipsize: gtk::pango::EllipsizeMode::End,
                            set_max_width_chars: 32,
                            set_margin_start: 16,
                            add_css_class: "heading",
                            #[watch]
                            set_label: &model.title,
                        },
                        gtk::Label {
                            add_css_class: "arut-availability",
                            set_wrap: true,
                            set_xalign: 0.0,
                            update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::LabelNodeAvailability))],
                            #[watch]
                            set_label: &model.availability,
                        },
                    },
                },
                gtk::Box {
                    set_vexpand: true,
                    #[name = "sidebar"]
                    gtk::Revealer {
                        set_transition_type: gtk::RevealerTransitionType::SlideRight,
                        // GtkRevealer skips the transition on its own when the
                        // desktop has animations off; there is no second path.
                        set_transition_duration: 200,
                        #[watch]
                        set_reveal_child: !model.navigation.collapsed,
                        #[watch]
                        set_hexpand: model.narrow && !model.navigation.collapsed,
                        #[local_ref]
                        conversations -> gtk::Box {},
                    },
                    #[name = "chat_surface"]
                    gtk::Box {
                        set_hexpand: true,
                        add_css_class: "arut-chat-surface",
                        #[watch]
                        set_visible: !model.narrow || model.navigation.collapsed,
                        #[name = "body"]
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 16,
                            set_margin_top: 16,
                            set_hexpand: true,
                            #[local_ref]
                            transcript -> gtk::Box {},
                            #[local_ref]
                            composer -> gtk::Box {},
                        },
                    },
                },
                gtk::Box {
                    #[watch]
                    set_visible: !model.error.is_empty(),
                    set_spacing: 8,
                    set_margin_start: 16,
                    set_margin_end: 16,
                    set_margin_bottom: 16,
                    add_css_class: "arut-error",
                    set_accessible_role: gtk::AccessibleRole::Alert,
                    gtk::Image {
                        set_icon_name: Some("dialog-warning-symbolic"),
                        set_accessible_role: gtk::AccessibleRole::Presentation,
                    },
                    gtk::Label {
                        #[watch]
                        set_label: &model.error,
                        set_wrap: true,
                        set_xalign: 0.0,
                        set_hexpand: true,
                    },
                },
            },
        }
    }
    fn init(
        session: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let conversations = Conversations::builder().launch(session.clone()).detach();
        let transcript = Transcript::builder()
            .launch(session.chat())
            .forward(sender.input_sender(), |Position(id, value)| {
                Msg::Position(id, value)
            });
        let composer = Composer::builder().launch(session.chat()).detach();
        let mut tasks = Tasks::default();
        for (changes, message) in [
            (session.conversations_changes(), Msg::Conversations),
            (session.availability_changes(), Msg::Availability),
        ] {
            let input = sender.input_sender().clone();
            tasks.observe(changes, move || {
                let _ = input.send(message.clone());
            });
        }
        let start = session.clone();
        let input = sender.input_sender().clone();
        tasks.spawn(async move {
            start.refresh_capabilities().await;
            let _ = input.send(Msg::Initialized(start.initialize().await));
        });
        // The mounted conversation and the session's selection start in step;
        // a saved selection is restored once the session has initialized.
        session.select(session.chat().id());
        let mut model = Self {
            displayed: Rc::new(RefCell::new(session.chat().id())),
            session,
            navigation: Navigation::load(),
            conversations,
            transcript,
            composer,
            body: gtk::Box::new(gtk::Orientation::Vertical, 0),
            drafts: HashMap::new(),
            positions: HashMap::new(),
            narrow: false,
            title: strings::show(&Message::ActionNewConversation),
            availability: String::new(),
            available: false,
            restore_pending: true,
            error: String::new(),
            _tasks: tasks,
            _chat_tasks: Tasks::default(),
            _theme: crate::app::theme::Theme::install(&root),
            _decorations: None,
        };
        let conversations = model.conversations.widget();
        let transcript = model.transcript.widget();
        let composer = model.composer.widget();
        let widgets = view_output!();
        model.body = widgets.body.clone();
        let pending = model.session.chat();
        model.follow_displayed(pending);
        model._decorations = Some(crate::app::decorations::Decorations::install(
            &root,
            &widgets.header,
            &widgets.toolbar,
        ));
        widgets.chat_surface.remove(&widgets.body);
        widgets
            .chat_surface
            .append(&crate::app::layout::Column::new(&widgets.body, 880));
        let content = root.child().unwrap();
        root.set_child(None::<&gtk::Widget>);
        let responsive = crate::app::layout::Column::new(&content, i32::MAX);
        responsive.on_breakpoint({
            let sender = sender.clone();
            move |narrow| sender.input(Msg::Narrow(narrow))
        });
        root.set_child(Some(&responsive));
        let latest = gio::SimpleAction::new("latest", None);
        latest.connect_activate({
            let transcript = model.transcript.sender().clone();
            move |_, _| {
                let _ = transcript.send(TranscriptMsg::Latest);
            }
        });
        root.add_action(&latest);
        let shortcuts = gtk::ShortcutController::new();
        shortcuts.set_scope(gtk::ShortcutScope::Managed);
        for (name, trigger, message) in [
            ("new", "<Control>n", Msg::New),
            ("sidebar", "<Control>b", Msg::ToggleSidebar),
            ("composer", "<Control>k", Msg::FocusComposer),
            ("search", "<Control>f", Msg::Search),
            ("escape", "Escape", Msg::Escape),
        ] {
            let action = gio::SimpleAction::new(name, None);
            let input = sender.input_sender().clone();
            action.connect_activate(move |_, _| {
                let _ = input.send(message.clone());
            });
            root.add_action(&action);
            shortcuts.add_shortcut(gtk::Shortcut::new(
                gtk::ShortcutTrigger::parse_string(trigger),
                Some(gtk::NamedAction::new(&format!("win.{name}"))),
            ));
        }
        root.add_controller(shortcuts);
        widgets.sidebar_toggle.connect_clicked({
            let sender = sender.clone();
            move |_| sender.input(Msg::ToggleSidebar)
        });
        #[cfg(feature = "review")]
        crate::app::review::install(&root, model.session.clone(), sender.clone());
        root.connect_map(|window| {
            eprintln!("arut-linux: shell mapped");
            if let Some(clock) = window.frame_clock() {
                let handler = Rc::new(RefCell::new(None));
                let disconnect = handler.clone();
                *handler.borrow_mut() = Some(clock.connect_after_paint(move |clock| {
                    eprintln!("arut-linux: first frame painted");
                    if let Some(handler) = disconnect.borrow_mut().take() {
                        clock.disconnect(handler);
                    }
                }));
            }
        });
        ComponentParts { model, widgets }
    }
    fn update(&mut self, message: Msg, _sender: ComponentSender<Self>, root: &Self::Root) {
        match message {
            Msg::New => {
                self.restore_pending = false;
                let chat = self.session.new_chat();
                self.session.select(chat.id());
                self.show(chat);
            }
            Msg::Conversations => {
                let selected = self.session.selected_id();
                if selected != *self.displayed.borrow() {
                    self.restore_pending = false;
                    let chat = match &selected {
                        Some(id) => self.session.select_chat(id),
                        None => Some(self.session.chat()),
                    };
                    if let Some(chat) = chat {
                        self.show(chat);
                    }
                }
                if !self.restore_pending && self.navigation.selected != selected {
                    self.navigation.selected = selected;
                    self.navigation.save();
                }
                self.retitle();
            }
            Msg::Position(id, value) => {
                self.positions.insert(id, value);
            }
            Msg::ToggleSidebar => {
                self.navigation.collapsed = !self.navigation.collapsed;
                self.navigation.save();
            }
            Msg::FocusComposer => {
                if self.narrow {
                    self.navigation.collapsed = true;
                }
                self.composer.emit(ComposerMsg::Focus);
            }
            Msg::Search => {
                self.navigation.collapsed = false;
                // The child grabs focus from its own update, after this one has
                // revealed the sidebar.
                self.conversations.emit(ConversationsMsg::Focus);
            }
            Msg::Escape => {
                let search = &self.conversations.model().search;
                if search.text().is_empty() {
                    GtkWindowExt::set_focus(root, None::<&gtk::Widget>);
                } else {
                    search.set_text("");
                }
            }
            Msg::Narrow(narrow) => {
                self.narrow = narrow;
                self.navigation.collapsed = narrow;
            }
            Msg::Availability => {
                let value = self.session.availability().composer;
                self.availability = strings::availability(value);
                self.available = value == FeatureAvailability::Available;
                self.composer.emit(ComposerMsg::Enabled(self.available));
            }
            Msg::Initialized(Ok(())) => {
                if self.restore_pending {
                    self.restore_pending = false;
                    let restored = self
                        .navigation
                        .selected
                        .clone()
                        .filter(|id| self.session.select_chat(id).is_some());
                    self.session.select(restored);
                }
            }
            Msg::Initialized(Err(error)) => self.error = strings::rpc(&error),
        }
    }
}

impl Shell {
    /// The conversation title, or the new-conversation label while the pending
    /// conversation has no messages of its own yet.
    fn retitle(&mut self) {
        let displayed = self.displayed.borrow().clone();
        self.title = self
            .session
            .chat_summaries()
            .into_iter()
            .find(|summary| Some(&summary.id) == displayed.as_ref())
            .map_or_else(
                || strings::show(&Message::ActionNewConversation),
                |summary| summary.title,
            );
    }

    /// Follows the mounted conversation so a pending one that becomes
    /// established updates the mount key and the session's selection together,
    /// which keeps `Msg::Conversations` from treating it as a new selection.
    fn follow_displayed(&mut self, chat: ChatClient) {
        let mut tasks = Tasks::default();
        let (displayed, session) = (self.displayed.clone(), self.session.clone());
        tasks.observe(chat.changes(), move || {
            let Some(id) = chat.id() else { return };
            if displayed.borrow().as_deref() != Some(id.as_str()) {
                *displayed.borrow_mut() = Some(id.clone());
                session.select(Some(id));
            }
        });
        self._chat_tasks = tasks;
    }

    /// Shows `chat`. The transcript is told to follow it; the composer is
    /// swapped for the one this conversation was last edited in, so its
    /// `TextBuffer` and undo history survive the switch.
    fn show(&mut self, chat: ChatClient) {
        let key = chat.id();
        let previous = self.displayed.replace(key.clone());
        if previous == key {
            if self.narrow {
                self.navigation.collapsed = true;
            }
            self.composer.emit(ComposerMsg::Focus);
            return;
        }
        self.transcript
            .emit(TranscriptMsg::Chat(Box::new(chat.clone())));
        if let Some(position) = self.positions.get(&key) {
            self.transcript.emit(TranscriptMsg::Restore(*position));
        }
        let next = self
            .drafts
            .remove(&key)
            .unwrap_or_else(|| Composer::builder().launch(chat.clone()).detach());
        let previous_composer = std::mem::replace(&mut self.composer, next);
        self.body.remove(previous_composer.widget());
        self.drafts.insert(previous, previous_composer);
        self.body.append(self.composer.widget());
        self.follow_displayed(chat);
        if self.narrow {
            self.navigation.collapsed = true;
        }
        self.composer.emit(ComposerMsg::Enabled(self.available));
        self.composer.emit(ComposerMsg::Focus);
        self.retitle();
    }
}
