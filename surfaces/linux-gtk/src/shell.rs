use crate::{
    availability::Availability,
    composer::{Composer, Msg as ComposerMsg},
    conversations::{Conversations, Msg as ConversationsMsg},
    navigation::Navigation,
    observe::Tasks,
    strings,
    transcript::Transcript,
};
use arut_feature_chat::product::ChatClient;
use arut_product_session::{FeatureAvailability, ProductSession};
use gtk::{glib, prelude::*};
use relm4::{Component, ComponentController, ComponentParts, ComponentSender, Controller};
use std::rc::Rc;

pub struct Shell {
    session: Rc<ProductSession>,
    navigation: Navigation,
    conversations: Controller<Conversations>,
    availability: Controller<Availability>,
    transcript: Controller<Transcript>,
    composer: Controller<Composer>,
    generation: u64,
    available: bool,
    restore_pending: bool,
    error: String,
    _tasks: Tasks,
    _theme: crate::theme::Theme,
}

#[derive(Debug)]
pub enum Msg {
    New,
    ToggleSidebar,
    FocusComposer,
    Select(String),
    ChatId(u64, Option<String>),
    Availability(FeatureAvailability),
    Initialized(Result<(), arut_rpc::Status>),
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
            set_title: Some("Arut"),
            add_css_class: "arut-window",
            set_default_size: (900, 600),
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 8,
                gtk::Box {
                    set_spacing: 6,
                    gtk::Button {
                        set_icon_name: "sidebar-show-symbolic",
                        set_tooltip_text: Some("Toggle sidebar (Ctrl+B)"),
                        update_property: &[gtk::accessible::Property::Label("Toggle sidebar")],
                        connect_clicked => Msg::ToggleSidebar,
                    },
                    gtk::Button {
                        set_icon_name: "list-add-symbolic",
                        set_tooltip_text: Some("New conversation (Ctrl+N)"),
                        update_property: &[gtk::accessible::Property::Label("New conversation")],
                        connect_clicked => Msg::New,
                    },
                    #[local_ref]
                    availability -> gtk::Label {},
                },
                gtk::Box {
                    set_spacing: 12,
                    set_vexpand: true,
                    #[local_ref]
                    conversations -> gtk::ScrolledWindow {
                        #[watch]
                        set_visible: !model.navigation.collapsed,
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
                gtk::Label {
                    #[watch]
                    set_label: &model.error,
                    set_wrap: true,
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
        let composer = Composer::builder().launch(session.chat()).detach();
        let mut tasks = Tasks::default();
        let initialize = session.clone();
        let input = sender.input_sender().clone();
        tasks.spawn(async move {
            let _ = input.send(Msg::Initialized(initialize.initialize().await));
        });
        let model = Self {
            session,
            navigation: Navigation::load(),
            conversations,
            availability,
            transcript,
            composer,
            generation: 0,
            available: false,
            restore_pending: true,
            error: String::new(),
            _tasks: tasks,
            _theme: crate::theme::Theme::install(&root),
        };
        let conversations = model.conversations.widget();
        let availability = model.availability.widget();
        let transcript = model.transcript.widget();
        let composer = model.composer.widget();
        let widgets = view_output!();
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            if !modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                return glib::Propagation::Proceed;
            }
            let message = match key {
                gtk::gdk::Key::n | gtk::gdk::Key::N => Msg::New,
                gtk::gdk::Key::b | gtk::gdk::Key::B => Msg::ToggleSidebar,
                gtk::gdk::Key::k | gtk::gdk::Key::K => Msg::FocusComposer,
                _ => return glib::Propagation::Proceed,
            };
            sender.input(message);
            glib::Propagation::Stop
        });
        root.add_controller(keys);
        root.connect_map(|window| {
            eprintln!("arut-linux-gtk: shell mapped");
            if let Some(clock) = window.frame_clock() {
                let handler = Rc::new(std::cell::RefCell::new(None));
                let disconnect = handler.clone();
                *handler.borrow_mut() = Some(clock.connect_after_paint(move |clock| {
                    eprintln!("arut-linux-gtk: first frame painted");
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
            Msg::FocusComposer => self.composer.emit(ComposerMsg::Focus),
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
        self.generation += 1;
        let generation = self.generation;
        self.select_navigation(chat.id());
        widgets.body.remove(self.transcript.widget());
        widgets.body.remove(self.composer.widget());
        self.transcript = Transcript::builder()
            .launch(chat.clone())
            .forward(sender.input_sender(), move |id| Msg::ChatId(generation, id));
        self.composer = Composer::builder().launch(chat).detach();
        self.composer.emit(ComposerMsg::Enabled(self.available));
        widgets.body.append(self.transcript.widget());
        widgets.body.append(self.composer.widget());
        self.composer.emit(ComposerMsg::Focus);
    }
}
