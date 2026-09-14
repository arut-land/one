use crate::app::{
    Session,
    composer::{Composer, Msg as ComposerMsg},
    conversations::{Conversations, Msg as ConversationsMsg},
    motion,
    navigation::Navigation,
    strings,
    transcript::{Msg as TranscriptMsg, Position, Transcript},
};
use crate::glib_observe::Tasks;
use arut_i18n::Message;
use arut_product_session::FeatureAvailability;
use arut_product_session::chat::ChatClient;
use gtk::{gio, glib, prelude::*};
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, RelmWidgetExt,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

/// The narrowest the conversation sidebar may be dragged: below this a title
/// and its preview stop being readable.
const SIDEBAR_MINIMUM: i32 = 220;

pub struct Shell {
    session: Rc<Session>,
    navigation: Navigation,
    conversations: Controller<Conversations>,
    transcript: Controller<Transcript>,
    composer: Controller<Composer>,
    body: gtk::Box,
    sidebar: gtk::Revealer,
    panes: gtk::Paned,
    /// The sidebar's slide in flight, if any; a new toggle replaces it.
    motion: Option<motion::Motion>,
    /// While the slide runs, pane position changes are its own and not a drag.
    animating: Rc<Cell<bool>>,
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
    /// `true` slides the sidebar; a layout-driven change passes `false` so a
    /// window crossing the breakpoint lands at once.
    ToggleSidebar(bool),
    /// The sidebar toggle button's state, which is a request only when it
    /// differs from the model's.
    SidebarButton(bool),
    /// Connect to the node again after a failed start.
    Retry,
    FocusComposer,
    Search,
    Escape,
    Narrow(bool),
    SidebarWidth(i32),
    Conversations,
    Position(Option<String>, f64),
    Availability,
    Initialized(Result<(), arut_product_session::SessionError>),
}

/// Every window shortcut in one place: the action it reaches, the triggers
/// that reach it, and the sentence the tooltip and the overlay both show. The
/// first trigger is the one rendered; the rest are aliases. `show-help-overlay`
/// is GTK's own action, so it appears here for its trigger and its label only.
fn shortcuts() -> [(&'static str, &'static [&'static str], Message); 6] {
    [
        ("new", &["<Control>n"], Message::ActionNewConversation),
        ("sidebar", &["<Control>b"], Message::ActionToggleSidebar),
        (
            "composer",
            &["<Control>l", "<Control>k"],
            Message::ActionFocusComposer,
        ),
        (
            "search",
            &["<Control>f"],
            Message::ActionSearchConversations,
        ),
        ("escape", &["Escape"], Message::ActionClearSearch),
        (
            "show-help-overlay",
            &["<Control>question"],
            Message::LabelKeyboardShortcuts,
        ),
    ]
}

/// The toggle shows what pressing it does: a panel closing while the sidebar
/// is shown, opening while it is hidden.
fn sidebar_icon(shown: bool) -> &'static str {
    if shown {
        "arut-sidebar-hide-symbolic"
    } else {
        "arut-sidebar-show-symbolic"
    }
}

/// The label a desktop shows for a trigger, from GTK rather than a literal:
/// modifier names are translated and platform-specific.
fn accelerator(display: &gtk::gdk::Display, trigger: &str) -> String {
    gtk::ShortcutTrigger::parse_string(trigger)
        .map(|trigger| trigger.to_label(display).to_string())
        .unwrap_or_default()
}

#[relm4::component(pub)]
impl Component for Shell {
    type Init = Rc<Session>;
    type Input = Msg;
    type Output = ();
    type CommandOutput = ();
    view! {
        #[name = "window"]
        gtk::ApplicationWindow {
            // The task switcher shows this, so it follows the conversation the
            // toolbar names rather than staying on the application name.
            #[watch]
            set_title: Some(&model.title),
            add_css_class: "arut-window",
            set_default_size: (model.navigation.size.0, model.navigation.size.1),
            set_maximized: model.navigation.maximized,
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
                            set_icon_name: sidebar_icon(!model.navigation.collapsed),
                            add_css_class: "flat",
                            #[watch]
                            set_active: !model.navigation.collapsed,
                            update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::ActionToggleSidebar))],
                        },
                        #[name = "new_conversation"]
                        gtk::Button {
                            set_icon_name: "arut-new-conversation-symbolic",
                            add_css_class: "flat",
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
                        // A node that is ready needs no announcement; the pill
                        // appears for the states that need explaining.
                        gtk::Label {
                            add_css_class: "arut-availability",
                            set_wrap: true,
                            set_xalign: 0.0,
                            set_valign: gtk::Align::Center,
                            update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::LabelNodeAvailability))],
                            #[watch]
                            set_label: &model.availability,
                            #[watch]
                            set_visible: !model.available && !model.availability.is_empty(),
                        },
                        #[name = "menu"]
                        gtk::MenuButton {
                            set_icon_name: "arut-menu-symbolic",
                            add_css_class: "flat",
                            set_tooltip_text: Some(&strings::show(&Message::ActionMainMenu)),
                            update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::ActionMainMenu))],
                        },
                    },
                },
                gtk::Overlay {
                    set_vexpand: true,
                    #[name = "panes"]
                    gtk::Paned {
                        set_vexpand: true,
                        set_orientation: gtk::Orientation::Horizontal,
                        // The chat column takes new width; the sidebar keeps the
                        // width the person gave it. The start child stays
                        // shrinkable so the slide can carry it to position zero
                        // and clip it on the way; the floor a drag stops at is
                        // `SIDEBAR_MINIMUM`, applied in `Msg::SidebarWidth`.
                        set_resize_start_child: false,
                        set_resize_end_child: true,
                        #[watch]
                        set_class_active: ("arut-collapsed", model.navigation.collapsed),
                        // The position is animated, so `collapse` owns it from
                        // here on rather than a watch re-applying it.
                        set_position: if model.navigation.collapsed { 0 } else { model.navigation.sidebar },
                        #[wrap(Some)]
                        #[name = "sidebar"]
                        set_start_child = &gtk::Revealer {
                            set_transition_type: gtk::RevealerTransitionType::Crossfade,
                            set_transition_duration: motion::DURATION_MS,
                            set_reveal_child: !model.navigation.collapsed,
                            #[local_ref]
                            conversations -> gtk::Box {},
                        },
                        #[wrap(Some)]
                        #[name = "chat_surface"]
                        set_end_child = &gtk::Box {
                            set_hexpand: true,
                            add_css_class: "arut-chat-surface",
                            #[watch]
                            set_visible: !model.narrow || model.navigation.collapsed,
                            #[name = "body"]
                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 12,
                                set_margin_top: 8,
                                set_hexpand: true,
                                #[local_ref]
                                transcript -> gtk::Box {},
                                #[local_ref]
                                composer -> gtk::Box {},
                            },
                        },
                    },
                    // A banner over whichever page is showing, the way the
                    // desktop's own apps announce a connection that failed.
                    add_overlay = &gtk::Revealer {
                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::Start,
                        set_margin_top: 8,
                        set_margin_start: 16,
                        set_margin_end: 16,
                        set_transition_type: gtk::RevealerTransitionType::SlideDown,
                        set_transition_duration: motion::DURATION_MS,
                        #[watch]
                        set_reveal_child: !model.error.is_empty(),
                        gtk::Box {
                            set_spacing: 8,
                            add_css_class: "arut-error",
                            set_accessible_role: gtk::AccessibleRole::Alert,
                            gtk::Image {
                                set_icon_name: Some("arut-warning-symbolic"),
                                set_accessible_role: gtk::AccessibleRole::Presentation,
                            },
                            gtk::Label {
                                #[watch]
                                set_label: &model.error,
                                set_wrap: true,
                                set_xalign: 0.0,
                                set_max_width_chars: 48,
                                set_valign: gtk::Align::Center,
                            },
                            gtk::Button {
                                set_label: &strings::show(&Message::ActionRetry),
                                add_css_class: "flat",
                                set_action_name: Some("win.retry"),
                            },
                        },
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
        let display = WidgetExt::display(&root);
        let mut navigation = Navigation::load();
        navigation.size = navigation.size_on(Some(&display));
        let mut model = Self {
            displayed: Rc::new(RefCell::new(session.chat().id())),
            session,
            navigation,
            conversations,
            transcript,
            composer,
            body: gtk::Box::new(gtk::Orientation::Vertical, 0),
            sidebar: gtk::Revealer::new(),
            panes: gtk::Paned::new(gtk::Orientation::Horizontal),
            motion: None,
            animating: Rc::new(Cell::new(false)),
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
        model.sidebar = widgets.sidebar.clone();
        model.panes = widgets.panes.clone();
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
        let retry = gio::SimpleAction::new("retry", None);
        retry.connect_activate({
            let input = sender.input_sender().clone();
            move |_, _| {
                let _ = input.send(Msg::Retry);
            }
        });
        root.add_action(&retry);
        let controller = gtk::ShortcutController::new();
        controller.set_scope(gtk::ShortcutScope::Managed);
        for (name, triggers, _) in shortcuts() {
            // `win.show-help-overlay` is GtkApplicationWindow's own action; the
            // table names it so it gets a trigger and an overlay entry.
            let message = match name {
                "new" => Some(Msg::New),
                "sidebar" => Some(Msg::ToggleSidebar(true)),
                "composer" => Some(Msg::FocusComposer),
                "search" => Some(Msg::Search),
                "escape" => Some(Msg::Escape),
                _ => None,
            };
            if let Some(message) = message {
                let action = gio::SimpleAction::new(name, None);
                let input = sender.input_sender().clone();
                action.connect_activate(move |_, _| {
                    let _ = input.send(message.clone());
                });
                root.add_action(&action);
            }
            for trigger in triggers {
                controller.add_shortcut(gtk::Shortcut::new(
                    gtk::ShortcutTrigger::parse_string(trigger),
                    Some(gtk::NamedAction::new(&format!("win.{name}"))),
                ));
            }
        }
        root.add_controller(controller);
        install_menu(&root, &widgets.menu);
        install_overlay(&root, &display);
        // The second discoverability layer: the accelerator in the tooltip is
        // rendered from the registered trigger, never typed in.
        widgets.sidebar_toggle.set_tooltip_text(Some(&strings::show(
            &Message::ActionToggleSidebarShortcut {
                shortcut: accelerator(&display, "<Control>b"),
            },
        )));
        widgets
            .new_conversation
            .set_tooltip_text(Some(&strings::show(
                &Message::ActionNewConversationShortcut {
                    shortcut: accelerator(&display, "<Control>n"),
                },
            )));
        widgets.sidebar.set_visible(!model.navigation.collapsed);
        widgets.sidebar.connect_child_revealed_notify(|sidebar| {
            sidebar.set_visible(sidebar.is_child_revealed());
        });
        // `toggled` also fires when the watch above writes the model's state
        // back into the button, so the handler reports the button's state and
        // `update` acts only when it disagrees with the model. Reacting to
        // `clicked` here would bounce the sidebar once per toggle.
        widgets.sidebar_toggle.connect_toggled({
            let sender = sender.clone();
            move |button| {
                button.set_icon_name(sidebar_icon(button.is_active()));
                sender.input(Msg::SidebarButton(button.is_active()));
            }
        });
        widgets.panes.connect_position_notify({
            let sender = sender.clone();
            move |panes| sender.input(Msg::SidebarWidth(panes.position()))
        });
        // Wayland owns placement, so the size and the pane width are what
        // there is to restore. They are read back off the widgets here rather
        // than mirrored into the model on every drag.
        root.connect_close_request({
            let panes = widgets.panes.downgrade();
            move |window| {
                let mut navigation = Navigation::load();
                navigation.size = window.default_size();
                navigation.maximized = window.is_maximized();
                if let Some(panes) = panes.upgrade().filter(|panes| panes.position() > 0) {
                    navigation.sidebar = panes.position();
                }
                navigation.save();
                glib::Propagation::Proceed
            }
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
    fn update(&mut self, message: Msg, sender: ComponentSender<Self>, root: &Self::Root) {
        match message {
            Msg::Retry => {
                self.error.clear();
                let session = self.session.clone();
                let input = sender.input_sender().clone();
                self._tasks.spawn(async move {
                    let _ = input.send(Msg::Initialized(session.initialize().await));
                });
            }
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
            Msg::SidebarWidth(width) => {
                if !self.animating.get() && !self.navigation.collapsed && width > 0 {
                    // Re-applying the floor is what stops a drag there rather
                    // than clipping the list.
                    let floor = width.max(SIDEBAR_MINIMUM);
                    self.navigation.sidebar = floor;
                    if floor != width {
                        self.panes.set_position(floor);
                    }
                }
            }
            Msg::ToggleSidebar(animate) => {
                self.collapse(!self.navigation.collapsed, animate);
                self.navigation.save();
            }
            Msg::SidebarButton(shown) => {
                if shown == self.navigation.collapsed {
                    self.collapse(!shown, true);
                    self.navigation.save();
                }
            }
            Msg::FocusComposer => {
                if self.narrow {
                    self.collapse(true, false);
                }
                self.composer.emit(ComposerMsg::Focus);
            }
            Msg::Search => {
                self.collapse(false, false);
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
                self.collapse(narrow, false);
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

/// The primary menu GNOME expects in the header: what this window can do that
/// is not a toolbar button. `win.show-help-overlay` is GTK's, `app.quit` the
/// application's; both are named here and nowhere else.
fn install_menu(window: &gtk::ApplicationWindow, button: &gtk::MenuButton) {
    let menu = gio::Menu::new();
    for (label, action) in [
        (Message::ActionAbout, "win.about"),
        (Message::LabelKeyboardShortcuts, "win.show-help-overlay"),
        (Message::ActionQuit, "app.quit"),
    ] {
        menu.append(Some(&strings::show(&label)), Some(action));
    }
    button.set_menu_model(Some(&menu));
    let about = gio::SimpleAction::new("about", None);
    about.connect_activate({
        let window = window.downgrade();
        move |_, _| {
            let Some(window) = window.upgrade() else {
                return;
            };
            gtk::AboutDialog::builder()
                .transient_for(&window)
                .modal(true)
                .program_name(strings::show(&Message::AppName))
                .version(env!("CARGO_PKG_VERSION"))
                .logo_icon_name("dev.arut.Arut")
                .build()
                .present();
        }
    });
    window.add_action(&about);
    let quit = gio::SimpleAction::new("quit", None);
    quit.connect_activate(|_, _| relm4::main_application().quit());
    relm4::main_application().add_action(&quit);
}

/// The third discoverability layer, built from the same table as the
/// accelerators and the tooltips, so a shortcut cannot exist without appearing
/// here. `GtkShortcutsWindow` is deprecated in GTK 4.18 in favour of
/// `AdwShortcutsDialog`, which this surface cannot use: ADR 0020 keeps it on
/// plain GTK4. It is also the only overlay `set_help_overlay` accepts, which
/// is what supplies `win.show-help-overlay`. The deprecation is allowed here
/// and nowhere else.
#[expect(deprecated)]
fn install_overlay(window: &gtk::ApplicationWindow, display: &gtk::gdk::Display) {
    let group = gtk::ShortcutsGroup::builder()
        .title(strings::show(&Message::LabelShortcutsGeneral))
        .build();
    for (_, triggers, message) in shortcuts() {
        group.append(
            &gtk::ShortcutsShortcut::builder()
                .title(strings::show(&message))
                // Space-separated alternatives, in GtkShortcutsShortcut's own
                // accelerator syntax.
                .accelerator(triggers.join(" "))
                .build(),
        );
    }
    let section = gtk::ShortcutsSection::builder().build();
    section.append(&group);
    let overlay = gtk::ShortcutsWindow::builder()
        .modal(true)
        .display(display)
        .title(strings::show(&Message::LabelKeyboardShortcuts))
        .build();
    overlay.add_section(&section);
    window.set_help_overlay(Some(&overlay));
}

impl Shell {
    /// Slides the sidebar in or out.
    ///
    /// The pane position eases between zero and the saved width on the frame
    /// clock while the `GtkRevealer` crossfades over the same duration, so
    /// the list is carried and faded rather than cut. A crossfading revealer
    /// keeps its child's width for the whole transition, so the widget itself
    /// is taken out of the layout once the fade has finished -- see the
    /// `child-revealed` handler in `init` -- and put back here before the
    /// next reveal starts.
    fn collapse(&mut self, collapsed: bool, animate: bool) {
        self.navigation.collapsed = collapsed;
        let target = if collapsed {
            0
        } else {
            self.navigation.sidebar
        };
        let from = self.panes.position();
        let animate = animate && motion::enabled(&self.panes);
        self.motion = None;
        self.animating.set(false);
        self.sidebar
            .set_transition_duration(if animate { motion::DURATION_MS } else { 0 });
        if !collapsed {
            self.sidebar.set_visible(true);
        }
        self.sidebar.set_reveal_child(!collapsed);
        if !animate || from == target {
            self.panes.set_position(target);
            return;
        }
        let (panes, animating) = (self.panes.clone(), self.animating.clone());
        animating.set(true);
        let distance = f64::from(target - from);
        self.motion = motion::run(&self.panes, motion::DURATION_MS, move |progress| {
            panes.set_position(from + (distance * progress).round() as i32);
            if progress >= 1.0 {
                animating.set(false);
            }
        });
    }

    /// The conversation title, or the new-conversation label while the pending
    /// conversation has no messages of its own yet. Rust decides which
    /// conversation that is; only the label for "none" is this surface's.
    fn retitle(&mut self) {
        self.title = self
            .session
            .selected_title()
            .unwrap_or_else(|| strings::show(&Message::ActionNewConversation));
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
                self.collapse(true, false);
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
            self.collapse(true, false);
        }
        self.composer.emit(ComposerMsg::Enabled(self.available));
        self.composer.emit(ComposerMsg::Focus);
        self.retitle();
    }
}
