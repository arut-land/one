//! Opt-in visual fixtures for review.sh. Actions never register in a normal run.
//! Messages go through the child node; only the typed error is injected.
use crate::shell::{Msg, Shell};
use arut_product_session::{ProductSession, SessionError};
use gtk::{gio, glib, prelude::*};
use relm4::ComponentSender;
use std::rc::Rc;

pub fn install(
    window: &gtk::ApplicationWindow,
    session: Rc<ProductSession>,
    sender: ComponentSender<Shell>,
) {
    if std::env::var("ARUT_REVIEW").as_deref() != Ok("1") {
        return;
    }
    let action = gio::SimpleAction::new("review", Some(glib::VariantTy::STRING));
    let weak = window.downgrade();
    action.connect_activate(move |_, parameter| {
        let Some(window) = weak.upgrade() else { return };
        let Some(scenario) = parameter.and_then(|v| v.str()) else { return };
        match scenario {
            "seed" => {
                let session = session.clone();
                let sender = sender.clone();
                glib::spawn_future_local(async move {
                    let other = session.new_chat();
                    other.send("Weekend plans".into()).await;
                    let chat = session.new_chat();
                    for index in 0..10 {
                        let text = if index == 0 {
                            "Visual review: a conversation about a quiet weekend".into()
                        } else if index == 9 {
                            "A longer message, with wrapping and a second paragraph. We can walk along the river, stop at the bakery, and bring a book to read under the trees. There is no need to hurry. ".repeat(4)
                                + "\n\nBefore we leave, check the weather and pack a light jacket."
                        } else {
                            format!("Weekend note {index}: leave time for a walk and a good conversation.")
                        };
                        chat.send(text).await;
                    }
                    assert_eq!(chat.messages_after(0).len(), 20, "review node replies");
                    sender.input(Msg::Select(chat.id().expect("persisted review chat")));
                    ready("seed");
                });
                return;
            }
            "one" | "many" => {
                let editor = descendant::<gtk::TextView>(&window).expect("composer");
                let text = if scenario == "one" {
                    "A short draft, ready to send.".into()
                } else {
                    (1..=10).map(|line| format!("Draft line {line}: keep this thought for later."))
                        .collect::<Vec<_>>().join("\n")
                };
                editor.buffer().set_text(&text);
            }
            "search" => {
                sender.input(Msg::Search);
                let search = descendant::<gtk::SearchEntry>(&window).expect("search");
                search.set_text("Visual review");
                search.emit_by_name::<()>("search-changed", &[]);
            }
            "clear-search" => {
                descendant::<gtk::SearchEntry>(&window).expect("search").set_text("");
            }
            "error" => sender.input(Msg::Initialized(Err(SessionError::Unavailable))),
            "scroll-up" => {
                let list = find(window.as_ref(), &|widget| {
                    widget.is::<gtk::ListView>() && widget.accessible_role() == gtk::AccessibleRole::Log
                }).expect("transcript");
                list.ancestor(gtk::ScrolledWindow::static_type()).unwrap()
                    .downcast::<gtk::ScrolledWindow>().unwrap().vadjustment().set_value(0.0);
            }
            _ => panic!("unknown review scenario: {scenario}"),
        }
        ready(scenario);
    });
    relm4::main_application().add_action(&action);
}
fn ready(scenario: &str) {
    eprintln!("arut-linux: review {scenario} ready");
}
fn descendant<T: IsA<gtk::Widget> + StaticType>(root: &impl IsA<gtk::Widget>) -> Option<T> {
    find(root.as_ref(), &|widget| widget.is::<T>()).and_then(|w| w.downcast().ok())
}
fn find(root: &gtk::Widget, matches: &impl Fn(&gtk::Widget) -> bool) -> Option<gtk::Widget> {
    if matches(root) {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(found) = find(&widget, matches) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}
