//! Display-backed checks of the watch/component boundary. No screenshots.
use crate::{
    availability::Availability,
    composer::{Composer, Msg as ComposerMsg},
    conversations::Conversations,
    transcript::Transcript,
};
use arut_product_session::ProductSession;
use gtk::{glib, prelude::*};
use relm4::{Component, ComponentController};
use std::{rc::Rc, sync::Arc};

fn drain(context: &glib::MainContext) {
    let mut iterations = 0;
    while context.pending() {
        context.iteration(false);
        iterations += 1;
        assert!(iterations < 10_000, "UI did not become idle");
    }
}

fn scroll_content(scroll: &gtk::ScrolledWindow) -> gtk::Widget {
    let child = scroll.child().unwrap();
    if let Some(viewport) = child.downcast_ref::<gtk::Viewport>() {
        viewport.child().unwrap()
    } else {
        child
    }
}

fn transcript_rows(transcript: &relm4::Controller<Transcript>) -> gtk::Box {
    let scroll = transcript
        .widget()
        .first_child()
        .unwrap()
        .downcast::<gtk::ScrolledWindow>()
        .unwrap();
    scroll_content(&scroll).downcast::<gtk::Box>().unwrap()
}

#[test]
#[ignore = "requires a GTK display; run with --ignored --test-threads=1"]
fn watches_preserve_message_widgets_and_bind_independent_drafts() {
    let _app = relm4::RelmApp::<()>::new("dev.arut.ComponentTest");
    let context = glib::MainContext::default();
    let _guard = context.acquire().unwrap();
    let session = Rc::new(ProductSession::local(
        "component-test",
        Arc::new(arut_feature_chat::ports::NativeIds),
    ));
    let chat = session.chat();
    let transcript = Transcript::builder().launch(chat.clone()).detach();
    let composer = Composer::builder().launch(chat.clone()).detach();
    let conversations = Conversations::builder().launch(session.clone()).detach();
    let availability = Availability::builder().launch(session.clone()).detach();
    drain(&context);
    assert_eq!(availability.widget().label(), "Ready");
    composer.emit(ComposerMsg::Enabled(true));
    drain(&context);
    let editor = composer
        .widget()
        .first_child()
        .unwrap()
        .downcast::<gtk::ScrolledWindow>()
        .unwrap()
        .child()
        .unwrap()
        .downcast::<gtk::TextView>()
        .unwrap();
    editor.buffer().set_text("first message");
    drain(&context);
    assert_eq!(chat.composer().state().text, "first message");
    let controllers = editor.observe_controllers();
    let keys = (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .filter_map(|object| object.downcast::<gtk::EventControllerKey>().ok())
        .find(|controller| controller.name().as_deref() == Some("arut-composer-keys"))
        .expect("composer keyboard controller");
    assert!(!keys.emit_by_name::<bool>(
        "key-pressed",
        &[
            &gtk::gdk::Key::Return,
            &0u32,
            &gtk::gdk::ModifierType::SHIFT_MASK
        ]
    ));
    drain(&context);
    assert!(
        chat.state().messages.is_empty(),
        "Shift+Enter must not send"
    );
    assert!(keys.emit_by_name::<bool>(
        "key-pressed",
        &[
            &gtk::gdk::Key::Return,
            &0u32,
            &gtk::gdk::ModifierType::empty()
        ]
    ));
    drain(&context);
    assert_eq!(chat.state().messages.len(), 2);
    assert_eq!(editor.buffer().char_count(), 0);
    let rows = transcript_rows(&transcript);
    let original = rows.first_child().unwrap();
    context.block_on(chat.send("second message".into()));
    drain(&context);
    assert_eq!(chat.state().messages.len(), 4);
    assert_eq!(
        rows.first_child().unwrap(),
        original,
        "appending must preserve the first message widget"
    );
    let history = scroll_content(conversations.widget())
        .downcast::<gtk::Box>()
        .unwrap();
    let original_conversation = history.first_child().unwrap();
    let second = session.new_chat();
    context.block_on(second.send("another conversation".into()));
    drain(&context);
    assert_eq!(
        history.last_child().unwrap(),
        original_conversation,
        "prepending must preserve the old conversation row"
    );
    context.block_on(second.composer().replace("other draft".into()));
    context.block_on(chat.composer().replace("updated draft".into()));
    drain(&context);
    let buffer = editor.buffer();
    assert_eq!(
        buffer.text(&buffer.start_iter(), &buffer.end_iter(), true),
        "updated draft"
    );
    assert_eq!(second.composer().state().text, "other draft");
    drop(composer);
    drain(&context);
    context.block_on(chat.composer().replace("after unmount".into()));
    drain(&context);
    assert_eq!(
        buffer.text(&buffer.start_iter(), &buffer.end_iter(), true),
        "updated draft",
        "unmounted composer must stop watching"
    );
}
