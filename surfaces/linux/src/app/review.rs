//! Opt-in visual fixtures for review.sh. Actions never register in a normal run.
//! Messages go through the child node; only the typed error is injected.
use crate::app::shell::{Msg, Shell};
use arut_product_session::{ProductSession, SessionError};
use gtk::{gio, glib, prelude::*};
use relm4::ComponentSender;
use std::{cell::RefCell, rc::Rc};

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
    let chats = Rc::new(RefCell::new(Vec::<String>::new()));
    action.connect_activate(move |_, parameter| {
        let Some(window) = weak.upgrade() else { return };
        let Some(scenario) = parameter.and_then(|v| v.str()) else { return };
        match scenario {
            "seed" => {
                let session = session.clone();
                let sender = sender.clone();
                let chats = chats.clone();
                glib::spawn_future_local(async move {
                    let other = session.new_chat();
                    other.send("Weekend plans".into()).await;
                    let chat = session.new_chat();
                    for index in 0..10 {
                        let text = if index == 0 {
                            "Saturday by the river".into()
                        } else if index == 9 {
                            "A longer message, with wrapping and a second paragraph. We can walk along the river, stop at the bakery, and bring a book to read under the trees. There is no need to hurry. ".repeat(4)
                                + "\n\nBefore we leave, check the weather and pack a light jacket."
                        } else {
                            [
                                "Saturday by the river",
                                "Shall we start at the bakery, then walk along the water?",
                                "I would like to be outside before the afternoon gets warm.",
                                "We could bring a picnic. Is there a quiet place to stop?",
                                "The park near the bridge has tables and a drinking fountain.",
                                "Let us take the path on the east bank. It has more shade.",
                                "I will bring sandwiches and fruit. Could you bring coffee?",
                                "If the weather changes, we can visit the museum nearby.",
                                "That sounds good. I will check the forecast on Friday.",
                            ][index].into()
                        };
                        chat.send(text).await;
                    }
                    assert_eq!(chat.messages_after(0).len(), 20, "review node replies");
                    *chats.borrow_mut() = vec![chat.id().unwrap(), other.id().unwrap()];
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
                editor.buffer().set_text("");
                for character in text.chars() {
                    editor.buffer().insert_at_cursor(&character.to_string());
                }
            }
            "switch-away" | "switch-back" => {
                let index = usize::from(scenario == "switch-away");
                sender.input(Msg::Select(chats.borrow()[index].clone()));
            }
            "groups" => {
                // The local echo node emits alternating pairs. Append display-only
                // continuations to exercise consecutive speakers without changing
                // the product's persisted protocol or the real send path.
                use arut_product_session::chat::{ChatMessage, ChatRole};
                let list = find(window.as_ref(), &|widget| {
                    widget.is::<gtk::ListView>() && widget.accessible_role() == gtk::AccessibleRole::Log
                }).unwrap().downcast::<gtk::ListView>().unwrap();
                let model = list.model().unwrap().downcast::<gtk::NoSelection>().unwrap()
                    .model().unwrap().downcast::<crate::app::message_model::Messages>().unwrap();
                let time = glib::real_time() as u64 / 1000 + 300_000;
                model.refresh(|last| vec![
                    ChatMessage { id: last + 1, role: ChatRole::Assistant,
                        text: "The riverside route is a good choice. We can meet at the bakery at nine.".into(),
                        accepted_at_ms: time },
                    ChatMessage { id: last + 2, role: ChatRole::Assistant,
                        text: "Bring a light jacket. It is usually cooler near the water.\n\nIf it rains, we can spend the morning at the museum instead.".into(),
                        accepted_at_ms: time + 1_000 },
                    ChatMessage { id: last + 3, role: ChatRole::User,
                        text: "Nine works for me. I will bring coffee.".into(),
                        accepted_at_ms: time + 2_000 },
                    ChatMessage { id: last + 4, role: ChatRole::User,
                        text: "See you there!".into(), accepted_at_ms: time + 3_000 },
                ]);
            }
            "send" => {
                descendant::<gtk::TextView>(&window).expect("composer").activate_action("composer.send", None).unwrap();
            }
            "search" => {
                sender.input(Msg::Search);
                let search = descendant::<gtk::SearchEntry>(&window).expect("search");
                search.set_text("Saturday");
                search.emit_by_name::<()>("search-changed", &[]);
            }
            "clear-search" => {
                descendant::<gtk::SearchEntry>(&window).expect("search").set_text("");
            }
            "error" => sender.input(Msg::Initialized(Err(SessionError::Unavailable))),
            "latest" => sender.input(Msg::Latest),
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
