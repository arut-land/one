use crate::observe::Tasks;
use arut_product_session::{ChatSummary, ProductSession};
use gtk::prelude::*;
use relm4::{
    ComponentParts, ComponentSender, SimpleComponent,
    factory::{DynamicIndex, FactoryComponent, FactorySender, FactoryVecDeque},
};
use std::rc::Rc;

pub struct ConversationRow {
    summary: ChatSummary,
    selected: bool,
}

#[relm4::factory(pub)]
impl FactoryComponent for ConversationRow {
    type Init = ChatSummary;
    type Input = ();
    type Output = String;
    type CommandOutput = ();
    type ParentWidget = gtk::Box;
    view! {
        gtk::ToggleButton {
            #[watch]
            set_label: &self.summary.title,
            #[watch]
            set_active: self.selected,
            #[watch]
            update_property: &[gtk::accessible::Property::Label(&self.summary.title)],
            connect_clicked[sender, id = self.summary.id.clone()] => move |_| {
                let _ = sender.output(id.clone());
            },
        }
    }
    fn init_model(summary: ChatSummary, _: &DynamicIndex, _: FactorySender<Self>) -> Self {
        Self {
            summary,
            selected: false,
        }
    }
}

pub struct Conversations {
    session: Rc<ProductSession>,
    rows: FactoryVecDeque<ConversationRow>,
    selected: Option<String>,
    _tasks: Tasks,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Changed,
    Selected(Option<String>),
}

#[relm4::component(pub)]
impl SimpleComponent for Conversations {
    type Init = Rc<ProductSession>;
    type Input = Msg;
    type Output = String;
    view! {
        gtk::ScrolledWindow {
            set_vexpand: true,
            set_min_content_width: 220,
            set_hscrollbar_policy: gtk::PolicyType::Never,
            update_property: &[gtk::accessible::Property::Label("Conversations")],
            #[local_ref]
            rows -> gtk::Box { set_orientation: gtk::Orientation::Vertical, set_spacing: 4 },
        }
    }
    fn init(
        session: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let output = sender.output_sender().clone();
        let rows = FactoryVecDeque::builder()
            .launch_default()
            .forward(&output, |id| id);
        let mut tasks = Tasks::default();
        tasks.watch(
            session.conversations_changes(),
            sender.input_sender().clone(),
            Msg::Changed,
        );
        let model = Self {
            session,
            rows,
            selected: None,
            _tasks: tasks,
        };
        let rows = model.rows.widget();
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }
    fn update(&mut self, message: Msg, _: ComponentSender<Self>) {
        if let Msg::Selected(selected) = message {
            self.selected = selected;
        }
        let summaries = self.session.chat_summaries();
        let mut rows = self.rows.guard();
        // Preserve rows by conversation ID, including when the core changes order.
        for (position, summary) in summaries.iter().enumerate() {
            let found = rows.iter().position(|row| row.summary.id == summary.id);
            match found {
                Some(index) if index != position => rows.move_to(index, position),
                None => {
                    rows.insert(position, summary.clone());
                }
                _ => (),
            }
            let row = rows.get_mut(position).expect("reconciled row");
            row.summary.clone_from(summary);
            row.selected = self.selected.as_ref() == Some(&summary.id);
        }
        while rows.len() > summaries.len() {
            rows.pop_back();
        }
    }
}
