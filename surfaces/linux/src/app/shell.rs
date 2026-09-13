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
    search_focus: Rc<Cell<bool>>,
    generation: u64,
    available: bool,
    restore_pending: bool,
    error: String,
    _tasks: Tasks,
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
    Latest,
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
                set_spacing: 8,
                #[name = "header"]
                gtk::HeaderBar {
                    #[name = "toolbar"]
                    pack_start = &gtk::Box {
                    set_spacing: 6,
                    gtk::Button {
                        set_icon_name: "sidebar-show-symbolic",
                        set_tooltip_text: Some(&strings::show(&Message::ActionToggleSidebarShortcut { shortcut: "Ctrl+B".into() })),
                        update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::ActionToggleSidebar))],
                        set_action_name: Some("win.sidebar"),
                    },
                    gtk::Button {
                        set_icon_name: "list-add-symbolic",
                        set_tooltip_text: Some(&strings::show(&Message::ActionNewConversationShortcut { shortcut: "Ctrl+N".into() })),
                        update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::ActionNewConversation))],
                        set_action_name: Some("win.new"),
                    },
                    #[local_ref]
                    availability -> gtk::Label {},
                    },
                },
                gtk::Box {
                    set_spacing: 12,
                    set_vexpand: true,
                    #[name = "sidebar"]
                    gtk::Revealer {
                        set_transition_type: gtk::RevealerTransitionType::SlideRight,
                        #[watch]
                        set_reveal_child: !model.navigation.collapsed,
                        #[local_ref]
                        conversations -> gtk::Box {},
                    },
                    #[name = "body"]
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 8,
                        set_hexpand: true,
                        #[local_ref]
                        transcript -> gtk::Box {},
                        #[local_ref]
                        composer -> gtk::Box {},
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
        let initialize = session.clone();
        let input = sender.input_sender().clone();
        tasks.spawn(async move {
            let _ = input.send(Msg::Initialized(initialize.initialize().await));
        });
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
            search_focus: Rc::default(),
            generation: 0,
            available: false,
            restore_pending: true,
            error: String::new(),
            _tasks: tasks,
            _theme: crate::app::theme::Theme::install(&root),
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
        let body_parent = widgets
            .body
            .parent()
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();
        body_parent.remove(&widgets.body);
        let column = crate::app::layout::Column::new(&widgets.body, 880);
        body_parent.append(&column);
        let content = root.child().unwrap();
        root.set_child(None::<&gtk::Widget>);
        let responsive = crate::app::layout::Column::new(&content, i32::MAX);
        responsive.on_breakpoint({
            let sender = sender.clone();
            move |narrow| sender.input(Msg::Narrow(narrow))
        });
        root.set_child(Some(&responsive));
        crate::app::theme::reveal_motion(&widgets.sidebar);
        widgets.sidebar.connect_child_revealed_notify({
            let request = model.search_focus.clone();
            let search = model.conversations.model().search.clone();
            move |sidebar| {
                if sidebar.is_child_revealed() && request.replace(false) {
                    search.grab_focus();
                }
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
                self.search_focus.set(true);
                if widgets.sidebar.is_child_revealed() {
                    self.search_focus.set(false);
                    self.conversations.model().search.grab_focus();
                }
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
                // Initial pending projection must not overwrite the saved selection before restore.
                if id.is_some() {
                    self.select_navigation(id);
                }
            }
            Msg::ChatId(_, _) => (),
            Msg::Availability(value) => {
                self.available = value == FeatureAvailability::Available;
                self.composer.emit(ComposerMsg::Enabled(self.available));
            }
            Msg::Initialized(result) => match result {
                Ok(()) => {
                    if self.restore_pending
                        && let Some(chat) = self
                            .navigation
                            .selected
                            .as_deref()
                            .and_then(|id| self.session.select_chat(id))
                    {
                        self.show_chat(chat, widgets, &sender);
                    }
                }
                Err(error) => self.error = strings::rpc(&error),
            },
        }
        widgets
            .sidebar
            .set_hexpand(self.narrow && !self.navigation.collapsed);
        widgets
            .body
            .parent()
            .unwrap()
            .set_visible(!self.narrow || self.navigation.collapsed);
        self.update_view(widgets, sender);
    }
}

impl Shell {
    fn select_navigation(&mut self, id: Option<String>) {
        self.conversations
            .emit(ConversationsMsg::Selected(id.clone()));
        if self.navigation.selected == id {
            return;
        }
        self.navigation.selected = id.clone();
        self.navigation.save();
    }
    fn show_chat(
        &mut self,
        chat: ChatClient,
        widgets: &ShellWidgets,
        sender: &ComponentSender<Self>,
    ) {
        let old_key = self
            .navigation
            .selected
            .clone()
            .unwrap_or_else(|| "pending".into());
        let new_key = chat.id().unwrap_or_else(|| "pending".into());
        if old_key == new_key && !self.restore_pending {
            self.composer.emit(ComposerMsg::Focus);
            return;
        }
        self.positions
            .insert(old_key.clone(), self.transcript.model().adjustment.value());
        self.generation += 1;
        let generation = self.generation;
        self.select_navigation(chat.id());
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
