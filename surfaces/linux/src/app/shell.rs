use crate::app::{
    availability::Availability,
    composer::{Composer, Msg as ComposerMsg},
    conversations::{Conversations, Msg as ConversationsMsg},
    navigation::Navigation,
    observe::Tasks,
    strings,
    transcript::Transcript,
};
use arut_i18n::Message;
use arut_product_session::chat::ChatClient;
use arut_product_session::{FeatureAvailability, ProductSession};
use gtk::{gio, prelude::*};
use relm4::{Component, ComponentController, ComponentParts, ComponentSender, Controller};
use std::{cell::Cell, collections::HashMap, rc::Rc};

pub struct Shell {
    session: Rc<ProductSession>,
    navigation: Navigation,
    conversations: Controller<Conversations>,
    availability: Controller<Availability>,
    transcript: Controller<Transcript>,
    composer: Controller<Composer>,
    drafts: HashMap<String, Controller<Composer>>,
    positions: HashMap<String, f64>,
    narrow: bool,
    displayed: Option<String>,
    generation: u64,
    available: bool,
    restore_pending: bool,
    error: String,
    _tasks: Tasks,
    theme: crate::app::theme::Theme,
    _decorations: Option<crate::app::decorations::Decorations>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    New,
    ToggleSidebar,
    ToggleSidebarAnimated,
    FocusComposer,
    Search,
    Escape,
    Latest,
    SummariesChanged,
    Narrow(bool),
    Select(String),
    ChatId(u64, Option<String>),
    Availability(FeatureAvailability),
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
                        set_spacing: 6,
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
                        #[name = "conversation_title"]
                        gtk::Label {
                            set_hexpand: true,
                            set_xalign: 0.0,
                            set_ellipsize: gtk::pango::EllipsizeMode::End,
                            set_max_width_chars: 32,
                            set_margin_start: 12,
                            add_css_class: "heading",
                        },
                        #[local_ref]
                        availability -> gtk::Label {},
                    },
                },
                gtk::Box {
                    set_vexpand: true,
                    #[name = "sidebar"]
                    gtk::Revealer {
                        set_transition_type: gtk::RevealerTransitionType::SlideRight,
                        set_transition_duration: 0,
                        #[watch]
                        set_reveal_child: !model.navigation.collapsed,
                        #[local_ref]
                        conversations -> gtk::Box {},
                    },
                    #[name = "chat_surface"]
                    gtk::Box {
                        set_hexpand: true,
                        add_css_class: "arut-chat-surface",
                        #[name = "body"]
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 12,
                            set_margin_top: 12,
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
                    set_margin_start: 12,
                    set_margin_end: 12,
                    set_margin_bottom: 12,
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
        let conversations = Conversations::builder()
            .launch(session.clone())
            .forward(sender.input_sender(), Msg::Select);
        let availability = Availability::builder()
            .launch(session.clone())
            .forward(sender.input_sender(), Msg::Availability);
        let transcript = Transcript::builder()
            .launch(session.chat())
            .forward(sender.input_sender(), |id| Msg::ChatId(0, id));
        let composer = Composer::builder()
            .launch(session.chat())
            .forward(sender.input_sender(), |()| Msg::Latest);
        let mut tasks = Tasks::default();
        tasks.observe(session.conversations_changes(), {
            let input = sender.input_sender().clone();
            move || {
                let _ = input.send(Msg::SummariesChanged);
            }
        });
        let initialize = session.clone();
        let input = sender.input_sender().clone();
        tasks.spawn(async move {
            let _ = input.send(Msg::Initialized(initialize.initialize().await));
        });
        let displayed = session.chat().id();
        let mut model = Self {
            session,
            navigation: Navigation::load(),
            conversations,
            availability,
            transcript,
            composer,
            drafts: HashMap::new(),
            positions: HashMap::new(),
            narrow: false,
            displayed,
            generation: 0,
            available: false,
            restore_pending: true,
            error: String::new(),
            _tasks: tasks,
            theme: crate::app::theme::Theme::install(&root),
            _decorations: None,
        };
        let conversations = model.conversations.widget();
        let availability = model.availability.widget();
        let transcript = model.transcript.widget();
        let composer = model.composer.widget();
        let widgets = view_output!();
        model._decorations = Some(crate::app::decorations::Decorations::install(
            &root,
            &widgets.header,
            &widgets.toolbar,
        ));
        widgets.chat_surface.remove(&widgets.body);
        let column = crate::app::layout::Column::new(&widgets.body, 880);
        widgets.chat_surface.append(&column);
        let content = root.child().unwrap();
        root.set_child(None::<&gtk::Widget>);
        let responsive = crate::app::layout::Column::new(&content, i32::MAX);
        responsive.on_breakpoint({
            let sender = sender.clone();
            move |narrow| sender.input(Msg::Narrow(narrow))
        });
        root.set_child(Some(&responsive));
        model.theme.bind_sidebar(&widgets.sidebar);
        let pointer_toggle = Rc::new(Cell::new(false));
        let input_source = gtk::EventControllerLegacy::new();
        input_source.set_propagation_phase(gtk::PropagationPhase::Capture);
        input_source.connect_event({
            let pointer = pointer_toggle.clone();
            move |_, event| {
                match event.event_type() {
                    gtk::gdk::EventType::ButtonPress | gtk::gdk::EventType::TouchBegin => {
                        pointer.set(true);
                    }
                    gtk::gdk::EventType::KeyPress
                    | gtk::gdk::EventType::TouchCancel
                    | gtk::gdk::EventType::LeaveNotify => pointer.set(false),
                    _ => (),
                }
                gtk::glib::Propagation::Proceed
            }
        });
        widgets.sidebar_toggle.add_controller(input_source);
        widgets.sidebar_toggle.connect_clicked({
            let sender = sender.clone();
            move |_| {
                sender.input(if pointer_toggle.replace(false) {
                    Msg::ToggleSidebarAnimated
                } else {
                    Msg::ToggleSidebar
                });
            }
        });
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
        crate::app::review::install(&root, model.session.clone(), sender.clone());
        root.connect_map(|window| {
            eprintln!("arut-linux: shell mapped");
            if let Some(clock) = window.frame_clock() {
                let handler = Rc::new(std::cell::RefCell::new(None));
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
    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Msg,
        sender: ComponentSender<Self>,
        _: &Self::Root,
    ) {
        let focus_search = matches!(message, Msg::Search);
        let sidebar_motion = matches!(message, Msg::ToggleSidebarAnimated) && !self.narrow;
        if matches!(
            message,
            Msg::ToggleSidebar
                | Msg::ToggleSidebarAnimated
                | Msg::Search
                | Msg::FocusComposer
                | Msg::Narrow(_)
                | Msg::New
                | Msg::Select(_)
        ) {
            self.theme.sidebar_motion(&widgets.sidebar, sidebar_motion);
        }
        match message {
            Msg::New => {
                self.restore_pending = false;
                self.show_chat(self.session.new_chat(), widgets, &sender);
            }
            Msg::Select(id) => {
                self.restore_pending = false;
                if let Some(chat) = self.session.select_chat(&id) {
                    self.show_chat(chat, widgets, &sender);
                }
            }
            Msg::ToggleSidebar | Msg::ToggleSidebarAnimated => {
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
            }
            Msg::Escape => {
                let search = &self.conversations.model().search;
                if !search.text().is_empty() {
                    search.set_text("");
                } else {
                    GtkWindowExt::set_focus(&widgets.window, None::<&gtk::Widget>);
                }
            }
            Msg::Latest => {
                let _ = self
                    .transcript
                    .widget()
                    .activate_action("transcript.latest", None);
            }
            Msg::Narrow(narrow) => {
                self.narrow = narrow;
                self.navigation.collapsed = narrow;
            }
            Msg::ChatId(generation, id) if generation == self.generation => {
                self.displayed.clone_from(&id);
                if !self.restore_pending {
                    self.select_navigation(id);
                }
            }
            Msg::SummariesChanged | Msg::ChatId(_, _) => (),
            Msg::Availability(value) => {
                self.available = value == FeatureAvailability::Available;
                self.composer.emit(ComposerMsg::Enabled(self.available));
            }
            Msg::Initialized(result) => match result {
                Ok(()) => {
                    if self.restore_pending {
                        self.restore_pending = false;
                        let chat = self
                            .navigation
                            .selected
                            .as_deref()
                            .and_then(|id| self.session.select_chat(id));
                        if let Some(chat) = chat {
                            self.show_chat(chat, widgets, &sender);
                        } else {
                            self.select_navigation(self.displayed.clone());
                        }
                    }
                }
                Err(error) => self.error = strings::rpc(&error),
            },
        }
        widgets
            .sidebar
            .set_hexpand(self.narrow && !self.navigation.collapsed);
        widgets
            .chat_surface
            .set_visible(!self.narrow || self.navigation.collapsed);
        let title = self
            .session
            .chat_summaries()
            .into_iter()
            .find(|summary| Some(&summary.id) == self.displayed.as_ref())
            .map_or_else(
                || strings::show(&Message::ActionNewConversation),
                |summary| summary.title,
            );
        widgets.conversation_title.set_label(&title);
        self.update_view(widgets, sender);
        if focus_search {
            self.conversations.model().search.grab_focus();
        }
    }
}

impl Shell {
    fn select_navigation(&mut self, id: Option<String>) {
        self.conversations
            .emit(ConversationsMsg::Selected(id.clone()));
        if self.navigation.selected == id {
            return;
        }
        self.navigation.selected = id;
        self.navigation.save();
    }
    fn show_chat(
        &mut self,
        chat: ChatClient,
        widgets: &ShellWidgets,
        sender: &ComponentSender<Self>,
    ) {
        let old_key = self.displayed.clone().unwrap_or_else(|| "pending".into());
        let new_key = chat.id().unwrap_or_else(|| "pending".into());
        if old_key == new_key {
            self.select_navigation(self.displayed.clone());
            if self.narrow {
                self.navigation.collapsed = true;
            }
            self.composer.emit(ComposerMsg::Focus);
            return;
        }
        self.positions
            .insert(old_key.clone(), self.transcript.model().adjustment.value());
        self.generation += 1;
        let generation = self.generation;
        self.displayed = chat.id();
        self.select_navigation(self.displayed.clone());
        widgets.body.remove(self.transcript.widget());
        widgets.body.remove(self.composer.widget());
        self.transcript = Transcript::builder()
            .launch(chat.clone())
            .forward(sender.input_sender(), move |id| Msg::ChatId(generation, id));
        let next = self.drafts.remove(&new_key).unwrap_or_else(|| {
            Composer::builder()
                .launch(chat)
                .forward(sender.input_sender(), |()| Msg::Latest)
        });
        let previous = std::mem::replace(&mut self.composer, next);
        self.drafts.insert(old_key, previous);
        if let Some(position) = self.positions.get(&new_key) {
            self.transcript
                .model()
                .restore_position
                .set(Some(*position));
        }
        if self.narrow {
            self.navigation.collapsed = true;
        }
        self.composer.emit(ComposerMsg::Enabled(self.available));
        widgets.body.append(self.transcript.widget());
        widgets.body.append(self.composer.widget());
        self.composer.emit(ComposerMsg::Focus);
    }
}
