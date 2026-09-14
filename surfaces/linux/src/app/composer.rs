use crate::app::strings;
use crate::glib_observe::Tasks;
use arut_i18n::Message;
use arut_product_session::chat::ChatClient;
use gtk::{gio, glib, prelude::*};
use relm4::{ComponentParts, ComponentSender, SimpleComponent};
use std::{cell::Cell, rc::Rc};

pub struct Composer {
    chat: ChatClient,
    state: ViewState,
    editor: gtk::TextView,
    _tasks: Tasks,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Send,
    Enabled(bool),
    Focus,
}

#[relm4::component(pub)]
impl SimpleComponent for Composer {
    type Init = ChatClient;
    type Input = Msg;
    type Output = ();
    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 8,
            set_margin_start: 16,
            set_margin_end: 16,
            set_margin_bottom: 16,
            gtk::Frame {
                add_css_class: "arut-composer",
                gtk::Box {
                    set_spacing: 8,
                    set_margin_start: 16,
                    set_margin_end: 8,
                    set_margin_top: 8,
                    set_margin_bottom: 8,
                    gtk::Overlay {
                        set_hexpand: true,
                        set_valign: gtk::Align::Center,
                        #[name = "scroll"]
                        gtk::ScrolledWindow {
                            set_hscrollbar_policy: gtk::PolicyType::Never,
                            set_propagate_natural_height: true,
                            #[local_ref]
                            editor -> gtk::TextView {
                                set_wrap_mode: gtk::WrapMode::WordChar,
                                set_accepts_tab: false,
                                set_top_margin: 8,
                                set_bottom_margin: 8,
                                update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::LabelDraft))],
                                set_tooltip_text: Some(&strings::show(&Message::ComposerHintMultiline)),
                            },
                        },
                        // Only an empty, single-line editor shows this, so it
                        // centres on that one line.
                        #[name = "placeholder"]
                        add_overlay = &gtk::Label {
                            set_label: &strings::show(&Message::ComposerPlaceholder),
                            set_halign: gtk::Align::Start,
                            set_valign: gtk::Align::Center,
                            set_can_target: false,
                            set_accessible_role: gtk::AccessibleRole::Presentation,
                            add_css_class: "dim-label",
                        },
                    },
                    #[name = "send"]
                    gtk::Button {
                        set_icon_name: "arut-send-symbolic",
                        set_valign: gtk::Align::End,
                        set_action_name: Some("composer.send"),
                        add_css_class: "suggested-action",
                        add_css_class: "circular",
                        update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::ActionSendMessage))],
                        set_tooltip_text: Some(&strings::show(&Message::ActionSendMessage)),
                    },
                },
            },
            #[name = "status"]
            gtk::Label {
                set_wrap: true,
                // A caption that appears and changes must be announced, which
                // is what the status role is for.
                set_accessible_role: gtk::AccessibleRole::Status,
                update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::LabelDraftSync))],
            },
        }
    }
    fn init(
        chat: ChatClient,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let composer = chat.composer();
        let mut model = Self {
            chat: chat.clone(),
            state: ViewState::default(),
            editor: gtk::TextView::new(),
            _tasks: Tasks::default(),
        };
        let editor = &model.editor;
        let widgets = view_output!();
        let buffer = model.editor.buffer();
        model
            .state
            .bind_property("enabled", &widgets.editor, "sensitive")
            .sync_create()
            .build();
        model
            .state
            .bind_property("can-send", &widgets.send, "sensitive")
            .sync_create()
            .build();
        model
            .state
            .bind_property("draft", &buffer, "text")
            .bidirectional()
            .sync_create()
            .build();
        model
            .state
            .bind_property("status", &widgets.status, "label")
            .sync_create()
            .build();
        model
            .state
            .bind_property("status", &widgets.status, "visible")
            .transform_to(|_, text: String| Some(!text.is_empty()))
            .sync_create()
            .build();
        model
            .state
            .bind_property("draft", &widgets.placeholder, "visible")
            .transform_to(|_, draft: String| Some(draft.is_empty()))
            .sync_create()
            .build();
        let initialize = composer.clone();
        model._tasks.spawn(async move {
            initialize.initialize().await;
            initialize.follow().await;
        });
        // Writing the draft back into the buffer must not be read as typing.
        let applying = Rc::new(Cell::new(false));
        model._tasks.observe(composer.changes(), {
            let (composer, state, chat, applying) = (
                composer.clone(),
                model.state.clone(),
                chat.clone(),
                applying.clone(),
            );
            move || {
                let snapshot = composer.state();
                state.set_status(strings::composer(snapshot.status, snapshot.error));
                if state.draft() != snapshot.text {
                    applying.set(true);
                    state.set_draft(snapshot.text.as_str());
                    applying.set(false);
                }
                sensitivity(&state, &chat, &snapshot.text);
            }
        });
        model._tasks.observe(chat.changes(), {
            let (state, chat) = (model.state.clone(), chat.clone());
            move || {
                let draft = state.draft();
                sensitivity(&state, &chat, &draft);
            }
        });
        buffer.connect_changed({
            let (state, chat) = (model.state.clone(), chat.clone());
            move |buffer| {
                if applying.get() {
                    return;
                }
                let text = buffer
                    .text(&buffer.start_iter(), &buffer.end_iter(), true)
                    .to_string();
                sensitivity(&state, &chat, &text);
                // `replace` writes the draft into the projection as it is
                // called and coalesces behind one in-flight request, so there
                // is no edit queue here. The write outlives this widget on
                // purpose: a draft typed just before a switch still reaches
                // the node.
                let composer = composer.clone();
                glib::spawn_future_local(async move {
                    composer.replace(text).await;
                });
            }
        });
        let actions = gio::SimpleActionGroup::new();
        let send = gio::SimpleAction::new("send", None);
        model
            .state
            .bind_property("can-send", &send, "enabled")
            .sync_create()
            .build();
        send.connect_activate(move |_, _| sender.input(Msg::Send));
        actions.add_action(&send);
        root.insert_action_group("composer", Some(&actions));
        let shortcuts = gtk::ShortcutController::new();
        for trigger in ["Return", "KP_Enter", "<Control>Return", "<Control>KP_Enter"] {
            shortcuts.add_shortcut(gtk::Shortcut::new(
                gtk::ShortcutTrigger::parse_string(trigger),
                Some(gtk::NamedAction::new("composer.send")),
            ));
        }
        widgets.editor.add_controller(shortcuts);
        // The editor's natural height is its one-line height, so the
        // scrolled window needs no minimum: an empty composer is exactly one
        // line tall and the send button sits level with it. Only the cap
        // needs the line height, read from the view's own layout when the
        // font can have changed and never on a scroll. It is a shared cell
        // rather than a local so the adjustment handler below reads the
        // same measurement.
        let line = Rc::new(Cell::new(1));
        let resize = {
            let line = line.clone();
            move |editor: &gtk::TextView, scroll: &gtk::ScrolledWindow| {
                // Pango's rounded pixel extents; truncating font metrics
                // clips the seventh baseline at fractional font sizes.
                let height = editor.create_pango_layout(Some("Ag")).pixel_size().1.max(1);
                line.set(height);
                // Bottom margin belongs to the end of the document, not every
                // viewport. Counting it here exposes part of an eighth line.
                scroll.set_max_content_height(height * 7 + editor.top_margin());
            }
        };
        resize(&widgets.editor, &widgets.scroll);
        // The font is final once the widget is mapped, which is when the
        // line height measured above is trustworthy.
        widgets.editor.connect_map({
            let scroll = widgets.scroll.clone();
            let resize = resize.clone();
            move |editor| resize(editor, &scroll)
        });
        // Keep the icon beside a short draft's first line. For a tall editor it
        // stays at the bottom, where sending does not interrupt the text column.
        widgets.scroll.vadjustment().connect_changed({
            let (send, line) = (widgets.send.downgrade(), line.clone());
            move |adjustment| {
                let Some(send) = send.upgrade() else { return };
                send.set_valign(if adjustment.page_size() <= f64::from(line.get() * 3) {
                    gtk::Align::Start
                } else {
                    gtk::Align::End
                });
            }
        });
        widgets.editor.connect_notify_local(Some("scale-factor"), {
            let scroll = widgets.scroll.clone();
            move |editor, _| resize(editor, &scroll)
        });
        ComponentParts { model, widgets }
    }
    fn update(&mut self, message: Msg, _sender: ComponentSender<Self>) {
        match message {
            Msg::Send if self.state.can_send() => {
                let text = self.state.draft();
                self.state.set_can_send(false);
                // `send` flushes the composer first, so the text committed is
                // the text on screen even with a write still in flight.
                let chat = self.chat.clone();
                glib::spawn_future_local(async move {
                    chat.send(text).await;
                });
                let _ = self.editor.activate_action("win.latest", None);
            }
            Msg::Send => (),
            Msg::Enabled(enabled) => {
                self.state.set_enabled(enabled);
                let draft = self.state.draft();
                sensitivity(&self.state, &self.chat, &draft);
            }
            Msg::Focus => {
                self.editor.grab_focus();
            }
        }
    }
}

/// A send is offered when the node would accept one and there is something to
/// send: `ChatState::can_send` covers the conversation scope and an in-flight
/// send, the draft covers the rest (ADR 0007 keeps the two projections apart).
fn sensitivity(state: &ViewState, chat: &ChatClient, draft: &str) {
    state.set_can_send(state.enabled() && chat.state().can_send && !draft.trim().is_empty());
}

mod properties {
    use gtk::{glib, prelude::*, subclass::prelude::*};
    use std::cell::{Cell, RefCell};

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::ViewState)]
    pub struct ViewState {
        #[property(get, set)]
        draft: RefCell<String>,
        #[property(get, set)]
        status: RefCell<String>,
        #[property(get, set)]
        enabled: Cell<bool>,
        #[property(get, set)]
        can_send: Cell<bool>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for ViewState {
        const NAME: &'static str = "ArutViewState";
        type Type = super::ViewState;
    }
    #[glib::derived_properties]
    impl ObjectImpl for ViewState {}
}

glib::wrapper! {
    /// The composer's presentation state: the two-way draft plus what the
    /// widgets around it derive from the chat and composer projections.
    pub struct ViewState(ObjectSubclass<properties::ViewState>);
}
impl Default for ViewState {
    fn default() -> Self {
        glib::Object::new()
    }
}
