use crate::{observe::Tasks, strings};
use arut_feature_chat::product::{ChatClient, ChatMessage, ChatStatus};
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
                set_label: strings::role(self.message.role),
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
    status: ChatStatus,
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
                update_property: &[gtk::accessible::Property::Label("Conversation transcript")],
                #[local_ref]
                rows -> gtk::Box { set_orientation: gtk::Orientation::Vertical, set_spacing: 16 },
            },
            gtk::Label {
                #[watch]
                set_label: strings::chat(model.status),
                update_property: &[gtk::accessible::Property::Label("Message status")],
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
        let model = Self {
            status: chat.state().status,
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
        let _ = sender.output(state.id);
        let removed: Vec<_> = self
            .rows
            .keys()
            .copied()
            .filter(|id| !state.messages.iter().any(|message| message.id == *id))
            .collect();
        for id in removed {
            self.rows.remove(&id);
        }
        for message in &state.messages {
            match self.rows.get(&message.id) {
                Some(row) if row.message != *message => {
                    self.rows
                        .get_mut(&message.id)
                        .expect("existing message")
                        .message
                        .clone_from(message);
                }
                None => {
                    self.rows.insert(message.id, message.clone());
                }
                _ => (),
            }
        }
        // HashMap keys retain component identity; GTK sibling order follows the projection.
        let container = self.rows.widget();
        let mut children = std::collections::HashMap::new();
        let mut child = container.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            children.insert(widget.widget_name().to_string(), widget);
        }
        let mut previous: Option<gtk::Widget> = None;
        for message in &state.messages {
            if let Some(widget) = children.remove(&message.id.to_string()) {
                container.reorder_child_after(&widget, previous.as_ref());
                previous = Some(widget);
            }
        }
    }
}
