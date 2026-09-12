use crate::{observe::Tasks, strings};
use arut_feature_chat::{
    composer::product::{ComposerClient, ComposerStatus},
    errors::ComposerError,
    product::ChatClient,
};
use arut_i18n::Message;
use gtk::{glib, prelude::*};
use relm4::{Component, ComponentParts, ComponentSender};
use std::{cell::Cell, rc::Rc};

pub struct Composer {
    chat: ChatClient,
    composer: ComposerClient,
    buffer: gtk::TextBuffer,
    applying: Rc<Cell<bool>>,
    enabled: bool,
    status: ComposerStatus,
    error: Option<ComposerError>,
    _tasks: Tasks,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Changed,
    Edit(String),
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
            gtk::ScrolledWindow {
                set_min_content_height: 90,
                set_max_content_height: 200,
                set_propagate_natural_height: true,
                #[name = "editor"]
                gtk::TextView {
                    set_buffer: Some(&model.buffer),
                    set_wrap_mode: gtk::WrapMode::WordChar,
                    set_accepts_tab: false,
                    #[watch]
                    set_sensitive: model.enabled,
                    update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::LabelDraft))],
                    set_tooltip_text: Some(&strings::show(&Message::ComposerHintMultiline)),
                },
            },
            gtk::Button {
                set_label: &strings::show(&Message::ActionSend),
                set_halign: gtk::Align::End,
                #[watch]
                set_sensitive: model.enabled,
                update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::ActionSendMessage))],
                connect_clicked => Msg::Send,
            },
            gtk::Label {
                #[watch]
                set_label: &strings::composer(model.status, model.error),
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
        let buffer = gtk::TextBuffer::default();
        buffer.set_text(&composer.state().text);
        let applying = Rc::new(Cell::new(false));
        let mut tasks = Tasks::default();
        tasks.watch(
            composer.changes(),
            sender.input_sender().clone(),
            Msg::Changed,
        );
        let initialize = composer.clone();
        tasks.spawn(async move {
            initialize.initialize().await;
            initialize.follow().await;
        });
        let initial = composer.state();
        let model = Self {
            chat,
            status: initial.status,
            error: initial.error,
            composer,
            buffer,
            applying,
            enabled: false,
            _tasks: tasks,
        };
        let widgets = view_output!();
        let input = sender.input_sender().clone();
        let applying = model.applying.clone();
        model.buffer.connect_changed(move |buffer| {
            if !applying.get() {
                let text = buffer
                    .text(&buffer.start_iter(), &buffer.end_iter(), true)
                    .to_string();
                let _ = input.send(Msg::Edit(text));
            }
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_name(Some("arut-composer-keys"));
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            if matches!(key, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter)
                && !modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK)
            {
                sender.input(Msg::Send);
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        widgets.editor.add_controller(keys);
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
            Msg::Changed => {
                let state = self.composer.state();
                self.status = state.status;
                self.error = state.error;
                let current =
                    self.buffer
                        .text(&self.buffer.start_iter(), &self.buffer.end_iter(), true);
                if current.as_str() != state.text {
                    self.applying.set(true);
                    self.buffer.set_text(&state.text);
                    self.applying.set(false);
                }
            }
            Msg::Edit(text) => {
                let composer = self.composer.clone();
                glib::spawn_future_local(async move {
                    composer.replace(text).await;
                });
            }
            Msg::Send if self.enabled => {
                let text = self
                    .buffer
                    .text(&self.buffer.start_iter(), &self.buffer.end_iter(), true)
                    .to_string();
                let chat = self.chat.clone();
                glib::spawn_future_local(async move {
                    chat.send(text).await;
                });
            }
            Msg::Send => (),
            Msg::Enabled(enabled) => self.enabled = enabled,
            Msg::Focus => {
                widgets.editor.grab_focus();
            }
        }
        self.update_view(widgets, sender);
    }
}
