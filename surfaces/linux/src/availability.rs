use crate::{
    observe::{Tasks, ViewState},
    strings,
};
use arut_i18n::Message;
use arut_product_session::{FeatureAvailability, ProductSession};
use gtk::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};
use std::rc::Rc;

pub struct Availability {
    _tasks: Tasks,
}
#[relm4::component(pub)]
impl SimpleComponent for Availability {
    type Init = Rc<ProductSession>;
    type Input = ();
    type Output = FeatureAvailability;
    view! {
        gtk::Label {
            add_css_class: "arut-availability",
            set_wrap: true,
            set_xalign: 0.0,
            update_property: &[gtk::accessible::Property::Label(&strings::show(&Message::LabelNodeAvailability))],
        }
    }
    fn init(
        session: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut model = Self {
            _tasks: Tasks::default(),
        };
        let widgets = view_output!();
        let state = ViewState::default();
        state
            .bind_property("status", &root, "label")
            .sync_create()
            .build();
        let refresh = session.clone();
        model._tasks.spawn(async move {
            refresh.refresh_capabilities().await;
        });
        let mut previous = None;
        model
            ._tasks
            .observe(session.availability_changes(), move || {
                let value = session.availability().composer;
                state.set_status(strings::availability(value));
                if previous != Some(value) {
                    previous = Some(value);
                    let _ = sender.output(value);
                }
            });
        ComponentParts { model, widgets }
    }
}
