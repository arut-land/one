use crate::{observe::Tasks, strings};
use arut_product_session::{FeatureAvailability, ProductSession};
use gtk::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};
use std::rc::Rc;

pub struct Availability {
    session: Rc<ProductSession>,
    value: FeatureAvailability,
    _tasks: Tasks,
}

#[relm4::component(pub)]
impl SimpleComponent for Availability {
    type Init = Rc<ProductSession>;
    type Input = ();
    type Output = FeatureAvailability;
    view! {
        gtk::Label {
            #[watch]
            set_label: strings::availability(model.value),
            set_wrap: true,
            set_xalign: 0.0,
            update_property: &[gtk::accessible::Property::Label("Node availability")],
        }
    }
    fn init(
        session: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut tasks = Tasks::default();
        tasks.watch(
            session.availability_changes(),
            sender.input_sender().clone(),
            (),
        );
        let refresh = session.clone();
        tasks.spawn(async move {
            refresh.refresh_capabilities().await;
        });
        let model = Self {
            value: session.availability().composer,
            session,
            _tasks: tasks,
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }
    fn update(&mut self, (): (), sender: ComponentSender<Self>) {
        self.value = self.session.availability().composer;
        let _ = sender.output(self.value);
    }
}
