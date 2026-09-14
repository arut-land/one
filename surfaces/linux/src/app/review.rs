//! Visual fixtures for tools/review-linux.sh, compiled only under the `review`
//! feature. Messages go through the child node; only the typed error is injected.
use crate::app::{
    Session,
    shell::{Msg, Shell},
    testing::{descendant, transcript},
};
use arut_product_session::SessionError;
use gtk::{gio, glib, prelude::*};
use relm4::ComponentSender;
use std::{cell::RefCell, rc::Rc};

/// One conversation long enough to exercise wrapping, grouping and scrolling.
const SEED: [&str; 9] = [
    "Saturday by the river",
    "Shall we start at the bakery, then walk along the water?",
    "I would like to be outside before the afternoon gets warm.",
    "We could bring a picnic. Is there a quiet place to stop?",
    "The park near the bridge has tables and a drinking fountain.",
    "Let us take the path on the east bank. It has more shade.",
    "I will bring sandwiches and fruit. Could you bring coffee?",
    "If the weather changes, we can visit the museum nearby.",
    "That sounds good. I will check the forecast on Friday.",
];

const LONG: &str = "A longer message, with wrapping and a second paragraph. We can walk along the river, stop at the bakery, and bring a book to read under the trees. There is no need to hurry. ";

pub fn install(
    window: &gtk::ApplicationWindow,
    session: Rc<Session>,
    sender: ComponentSender<Shell>,
) {
    let action = gio::SimpleAction::new("review", Some(glib::VariantTy::STRING));
    let weak = window.downgrade();
    let chats = Rc::new(RefCell::new(Vec::<String>::new()));
    action.connect_activate(move |_, parameter| {
        let Some(window) = weak.upgrade() else { return };
        let Some(scenario) = parameter.and_then(|value| value.str()) else {
            return;
        };
        match scenario {
            "seed" => {
                let (session, chats) = (session.clone(), chats.clone());
                glib::spawn_future_local(async move {
                    let other = session.new_chat();
                    other.send("Weekend plans".into()).await;
                    let chat = session.new_chat();
                    for index in 0..10 {
                        let text = match index {
                            9 => LONG.repeat(4)
                                + "\n\nBefore we leave, check the weather and pack a light jacket.",
                            index => SEED[index].to_owned(),
                        };
                        chat.send(text).await;
                    }
                    assert_eq!(chat.messages_after(0).len(), 20, "review node replies");
                    *chats.borrow_mut() = vec![chat.id().unwrap(), other.id().unwrap()];
                    session.select(Some(chat.id().expect("persisted review chat")));
                    ready("seed");
                });
                return;
            }
            "one" | "many" => {
                let editor = descendant::<gtk::TextView>(&window).expect("composer");
                let text = if scenario == "one" {
                    "A short draft, ready to send.".to_owned()
                } else {
                    (1..=10)
                        .map(|line| format!("Draft line {line}: keep this thought for later."))
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                editor.buffer().set_text("");
                for character in text.chars() {
                    editor.buffer().insert_at_cursor(&character.to_string());
                }
            }
            "switch-away" | "switch-back" => {
                let index = usize::from(scenario == "switch-away");
                session.select(Some(chats.borrow()[index].clone()));
            }
            "groups" => append_continuations(&window),
            "send" => {
                descendant::<gtk::TextView>(&window)
                    .expect("composer")
                    .activate_action("composer.send", None)
                    .unwrap();
            }
            "search" => {
                sender.input(Msg::Search);
                let search = descendant::<gtk::SearchEntry>(&window).expect("search");
                search.set_text("Saturday");
            }
            "sidebar-toggle" => sender.input(Msg::ToggleSidebar(true)),
            "focus-composer" => sender.input(Msg::FocusComposer),
            "typed" => {
                // What the Hyprland path types on a real keyboard, for a
                // backend that cannot inject keys.
                let editor = descendant::<gtk::TextView>(&window).expect("composer");
                editor.buffer().set_text("");
                for character in "hello".chars() {
                    editor.buffer().insert_at_cursor(&character.to_string());
                }
                editor.activate_action("composer.send", None).unwrap();
            }
            "menu" | "menu-close" => {
                let menu = descendant::<gtk::MenuButton>(&window).expect("primary menu");
                // A menu popover needs a keyboard grab, so it stays open only
                // on a backend whose window is focused. The Hyprland output
                // never takes focus, and captures the pressed button instead.
                if scenario == "menu" {
                    menu.popup();
                } else {
                    menu.popdown();
                }
            }
            "clear-search" => {
                descendant::<gtk::SearchEntry>(&window)
                    .expect("search")
                    .set_text("");
            }
            "error" => sender.input(Msg::Initialized(Err(SessionError::Unreachable))),
            "latest" => {
                let _ = WidgetExt::activate_action(&window, "win.latest", None);
            }
            "scroll-up" => {
                transcript(&window)
                    .expect("transcript")
                    .ancestor(gtk::ScrolledWindow::static_type())
                    .unwrap()
                    .downcast::<gtk::ScrolledWindow>()
                    .unwrap()
                    .vadjustment()
                    .set_value(0.0);
            }
            _ => panic!("unknown review scenario: {scenario}"),
        }
        ready(scenario);
    });
    relm4::main_application().add_action(&action);
}

/// The local echo node emits alternating pairs. Display-only continuations
/// exercise consecutive speakers without changing the persisted protocol or
/// the real send path.
fn append_continuations(window: &gtk::ApplicationWindow) {
    use arut_product_session::chat::{ChatMessage, ChatRole};
    let model = transcript(window)
        .unwrap()
        .model()
        .unwrap()
        .downcast::<gtk::NoSelection>()
        .unwrap()
        .model()
        .unwrap()
        .downcast::<crate::glib_observe::Rows>()
        .unwrap();
    let time = glib::real_time() as u64 / 1000 + 300_000;
    let appended = model.cursor() + 4;
    model.refresh(appended, |last| {
        vec![
            ChatMessage {
                id: last + 1,
                role: ChatRole::Assistant,
                text: "The riverside route is a good choice. We can meet at the bakery at nine."
                    .into(),
                accepted_at_ms: time,
                starts_time_group: true,
                starts_speaker_group: true,
                ends_speaker_group: false,
            },
            ChatMessage {
                id: last + 2,
                role: ChatRole::Assistant,
                text: "Bring a light jacket. It is usually cooler near the water.\n\nIf it rains, we can spend the morning at the museum instead.".into(),
                accepted_at_ms: time + 1_000,
                starts_time_group: false,
                starts_speaker_group: false,
                ends_speaker_group: true,
            },
            ChatMessage {
                id: last + 3,
                role: ChatRole::User,
                text: "Nine works for me. I will bring coffee.".into(),
                accepted_at_ms: time + 2_000,
                starts_time_group: false,
                starts_speaker_group: true,
                ends_speaker_group: false,
            },
            ChatMessage {
                id: last + 4,
                role: ChatRole::User,
                text: "See you there!".into(),
                accepted_at_ms: time + 3_000,
                starts_time_group: false,
                starts_speaker_group: false,
                ends_speaker_group: true,
            },
        ]
    });
}

fn ready(scenario: &str) {
    eprintln!("arut-linux: review {scenario} ready");
}
