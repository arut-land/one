use crate::{observe::Tasks, strings};
use arut_feature_chat::{
    errors::ChatError,
    product::{ChatClient, ChatMessage, ChatStatus},
};
use arut_i18n::Message;
use gtk::prelude::*;
use relm4::{
    ComponentParts, ComponentSender, SimpleComponent,
    factory::{FactoryComponent, FactoryHashMap, FactorySender},
};

pub struct MessageRow {
    message: ChatMessage,
}

#[relm4::factory(pub)]
impl FactoryComponent for MessageRow {
    type Init = ChatMessage;
    type Input = ();
    type Output = ();
    type CommandOutput = ();
    type ParentWidget = gtk::Box;
    type Index = u64;
    view! {
        gtk::Box {
            set_widget_name: &self.message.id.to_string(),
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 4,
            add_css_class: "arut-message",
            gtk::Label {
                set_xalign: 0.0,
                #[watch]
                set_label: &strings::role(self.message.role),
                add_css_class: "heading",
            },
            gtk::Label {
                set_xalign: 0.0,
                set_wrap: true,
                set_selectable: true,
                #[watch]
                set_label: &self.message.text,
            },
        }
    }
    fn init_model(message: ChatMessage, _: &u64, _: FactorySender<Self>) -> Self {
        Self { message }
    }
}

pub struct Transcript {
    chat: ChatClient,
    rows: FactoryHashMap<u64, MessageRow>,
    last_message_id: u64,
    status: ChatStatus,
    error: Option<ChatError>,
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
            gtk::ScrolledWindow {
                set_vexpand: true,
                set_hscrollbar_policy: gtk::PolicyType::Never,
                update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::LabelTranscript))],
                #[local_ref]
                rows -> gtk::Box { set_orientation: gtk::Orientation::Vertical, set_spacing: 16 },
            },
            gtk::Label {
                #[watch]
                set_label: &strings::chat(model.status, model.error),
                update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::LabelMessageStatus))],
            },
        }
    }
    fn init(
        chat: ChatClient,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let rows = FactoryHashMap::builder().launch_default().detach();
        let mut tasks = Tasks::default();
        tasks.watch(chat.changes(), sender.input_sender().clone(), ());
        let initial = chat.state();
        let model = Self {
            last_message_id: 0,
            status: initial.status,
            error: initial.error,
            chat,
            rows,
            _tasks: tasks,
        };
        let rows = model.rows.widget();
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }
    fn update(&mut self, (): (), sender: ComponentSender<Self>) {
        let state = self.chat.state();
        self.status = state.status;
        self.error = state.error;
        let _ = sender.output(state.id);
        for message in self.chat.messages_after(self.last_message_id) {
            self.last_message_id = message.id;
            self.rows.insert(message.id, message);
        }
    }
}
