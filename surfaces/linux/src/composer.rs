use crate::{
    observe::{Tasks, ViewState},
    strings,
};
use arut_i18n::Message;
use arut_product_session::chat::ChatClient;
use gtk::{gio, glib, prelude::*};
use relm4::{Component, ComponentParts, ComponentSender};
use std::{cell::Cell, rc::Rc};

pub struct Composer {
    state: ViewState,
    buffer: gtk::TextBuffer,
    commands: relm4::Sender<Command>,
    pending: Rc<Cell<usize>>,
    sending: Rc<Cell<bool>>,
    active: Rc<Cell<bool>>,
    edit_signal: Option<glib::SignalHandlerId>,
    _tasks: Tasks,
}

enum Command {
    Edit(String),
    Send(String),
}
#[derive(Debug, Clone)]
pub enum Msg {
    Send,
    Enabled(bool),
    Focus,
}

#[relm4::component(pub)]
impl Component for Composer {
    type Init = ChatClient;
    type Input = Msg;
    type Output = ();
    type CommandOutput = ();
    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 6,
            set_margin_start: 12,
            set_margin_end: 12,
            set_margin_bottom: 12,
            gtk::Frame {
                add_css_class: "arut-composer",
                gtk::Box {
                    set_spacing: 8,
                    set_margin_start: 12,
                    set_margin_end: 8,
                    set_margin_top: 8,
                    set_margin_bottom: 8,
                    gtk::Overlay {
                        set_hexpand: true,
                        #[name = "scroll"]
                        gtk::ScrolledWindow {
                            set_hscrollbar_policy: gtk::PolicyType::Never,
                            set_propagate_natural_height: true,
                            #[name = "editor"]
                            gtk::TextView {
                                set_buffer: Some(&model.buffer),
                                set_wrap_mode: gtk::WrapMode::WordChar,
                                set_accepts_tab: false,
                                update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::LabelDraft))],
                                set_tooltip_text: Some(&strings::show(&Message::ComposerHintMultiline)),
                            },
                        },
                        #[name = "placeholder"]
                        add_overlay = &gtk::Label {
                            set_label: &strings::show(&Message::ComposerPlaceholder),
                            set_halign: gtk::Align::Start,
                            set_valign: gtk::Align::Start,
                            set_can_target: false,
                            set_accessible_role: gtk::AccessibleRole::Presentation,
                            add_css_class: "dim-label",
                        },
                    },
                    #[name = "send"]
                    gtk::Button {
                        set_icon_name: "go-up-symbolic",
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
        let state = ViewState::default();
        let buffer = gtk::TextBuffer::default();
        let pending = Rc::new(Cell::new(0usize));
        let sending = Rc::new(Cell::new(false));
        let applying = Rc::new(Cell::new(false));
        let (commands, receiver) = relm4::channel();
        let mut model = Self {
            state,
            buffer,
            commands,
            pending,
            sending,
            active: Rc::new(Cell::new(true)),
            edit_signal: None,
            _tasks: Tasks::default(),
        };
        let widgets = view_output!();
        model
            .state
            .bind_property("draft", &model.buffer, "text")
            .bidirectional()
            .sync_create()
            .build();
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
            .bind_property("status", &widgets.status, "label")
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
        let refresh = {
            let active = model.active.clone();
            let (composer, state, pending, sending, applying) = (
                composer.clone(),
                model.state.clone(),
                model.pending.clone(),
                model.sending.clone(),
                applying.clone(),
            );
            move || {
                if !active.get() {
                    return;
                }
                let snapshot = composer.state();
                state.set_status(strings::composer(snapshot.status, snapshot.error));
                if pending.get() == 0 {
                    applying.set(true);
                    if state.draft() != snapshot.text {
                        state.set_draft(snapshot.text);
                    }
                    applying.set(false);
                }
                state.set_can_send(
                    state.enabled() && !sending.get() && !state.draft().trim().is_empty(),
                );
            }
        };
        model._tasks.observe(composer.changes(), refresh.clone());
        model.edit_signal = Some(model.buffer.connect_changed({
            let (commands, pending, state, sending) = (
                model.commands.clone(),
                model.pending.clone(),
                model.state.clone(),
                model.sending.clone(),
            );
            move |buffer| {
                if !applying.get() {
                    pending.set(pending.get() + 1);
                    let text = buffer
                        .text(&buffer.start_iter(), &buffer.end_iter(), true)
                        .to_string();
                    state
                        .set_can_send(state.enabled() && !sending.get() && !text.trim().is_empty());
                    let _ = commands.send(Command::Edit(text));
                }
            }
        }));
        // One consumer orders every edit and send. It drains accepted UI commands
        // when the component is dropped; observations still cancel immediately.
        let (pending, sending) = (model.pending.clone(), model.sending.clone());
        glib::spawn_future_local(async move {
            while let Some(command) = receiver.recv().await {
                match command {
                    Command::Edit(text) => {
                        composer.replace(text).await;
                    }
                    Command::Send(text) => {
                        chat.send(text).await;
                        sending.set(false);
                    }
                }
                pending.set(pending.get() - 1);
                refresh();
            }
        });
        let actions = gio::SimpleActionGroup::new();
        let send = gio::SimpleAction::new("send", None);
        model
            .state
            .bind_property("can-send", &send, "enabled")
            .sync_create()
            .build();
        send.connect_activate({
            let sender = sender.clone();
            move |_, _| sender.input(Msg::Send)
        });
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
        let resize = |editor: &gtk::TextView, scroll: &gtk::ScrolledWindow| {
            let metrics = editor.pango_context().metrics(None, None);
            let line = ((metrics.ascent() + metrics.descent()) / gtk::pango::SCALE).max(1);
            scroll.set_min_content_height(line);
            scroll.set_max_content_height(line * 7);
        };
        resize(&widgets.editor, &widgets.scroll);
        widgets.editor.connect_notify_local(Some("scale-factor"), {
            let scroll = widgets.scroll.clone();
            move |editor, _| resize(editor, &scroll)
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
            Msg::Send if self.state.can_send() => {
                let _ = sender.output(());
                self.sending.set(true);
                self.state.set_can_send(false);
                self.pending.set(self.pending.get() + 1);
                let _ = self.commands.send(Command::Send(self.state.draft()));
            }
            Msg::Send => (),
            Msg::Enabled(enabled) => {
                self.state.set_enabled(enabled);
                self.state.set_can_send(
                    enabled && !self.sending.get() && !self.state.draft().trim().is_empty(),
                );
            }
            Msg::Focus => {
                widgets.editor.grab_focus();
            }
        }
    }
}

impl Drop for Composer {
    fn drop(&mut self) {
        self.active.set(false);
        if let Some(signal) = self.edit_signal.take() {
            self.buffer.disconnect(signal);
        }
    }
}
