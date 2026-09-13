//! Display-backed checks of the watch/component boundary. No screenshots.
use crate::{
    availability::Availability,
    composer::{Composer, Msg as ComposerMsg},
    conversations::Conversations,
    transcript::Transcript,
};
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

fn wait_until(context: &glib::MainContext, ready: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        drain(context);
        if ready() {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "UI did not receive the node response"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

fn descendant<T: IsA<gtk::Widget> + StaticType + Clone>(widget: &impl IsA<gtk::Widget>) -> T {
    fn find<T: IsA<gtk::Widget> + StaticType + Clone>(widget: &gtk::Widget) -> Option<T> {
        if let Ok(found) = widget.clone().downcast::<T>() {
            return Some(found);
        }
        let mut child = widget.first_child();
        while let Some(current) = child {
            if let Some(found) = find(&current) {
                return Some(found);
            }
            child = current.next_sibling();
        }
        None
    }
    find(widget.as_ref()).expect("widget type in component")
}

#[test]
#[ignore = "requires a GTK display; run with --ignored --test-threads=1"]
fn recycled_models_search_and_ordered_drafts_work_over_ipc() {
    use arut_product_session::{ProductSession, SessionScope, hosting::Host};
    use arut_runtime_local::{
        child::ChildHost,
        hosting::{TokioSpawner, desktop_executor},
    };
    let _app = relm4::RelmApp::<()>::new("dev.arut.ComponentTest");
    let context = glib::MainContext::default();
    let _guard = context.acquire().unwrap();
    let executor = desktop_executor().unwrap();
    let directory = std::env::temp_dir().join(format!("arut-gtk-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    let host = ChildHost {
        executable: std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("arutd"),
        socket: directory.join("node.sock"),
        data: directory.join("data"),
        spawner: Arc::new(TokioSpawner(executor.handle().clone())),
    };
    let channel = executor.block_on(host.connect()).unwrap();
    let session = Rc::new(ProductSession::remote(
        channel,
        SessionScope {
            node_id: "local".into(),
            workspace_id: "default".into(),
            pending_scope_id: "component-test".into(),
        },
        Arc::new(arut_runtime_local::NativeIds),
    ));
    context.block_on(session.refresh_capabilities());
    let chat = session.chat();
    let transcript = Transcript::builder().launch(chat.clone()).detach();
    let composer = Composer::builder().launch(chat.clone()).detach();
    let conversations = Conversations::builder().launch(session.clone()).detach();
    let window = gtk::Window::new();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.append(transcript.widget());
    content.append(composer.widget());
    window.set_child(Some(&content));
    window.present();
    let availability = Availability::builder().launch(session.clone()).detach();
    drain(&context);
    assert_eq!(availability.widget().label(), "Ready");
    composer.emit(ComposerMsg::Enabled(true));
    drain(&context);
    let editor: gtk::TextView = descendant(composer.widget());
    editor.buffer().set_text("first message");
    wait_until(&context, || chat.composer().state().text == "first message");
    assert_eq!(chat.composer().state().text, "first message");
    let controllers = editor.observe_controllers();
    let shortcuts = (0..controllers.n_items())
        .filter_map(|p| controllers.item(p))
        .filter_map(|o| o.downcast::<gtk::ShortcutController>().ok())
        .find(|c| c.n_items() == 4)
        .expect("composer shortcuts");
    let shortcut = shortcuts
        .item(0)
        .unwrap()
        .downcast::<gtk::Shortcut>()
        .unwrap();
    let trigger = shortcut
        .trigger()
        .unwrap()
        .downcast::<gtk::KeyvalTrigger>()
        .unwrap();
    assert_eq!(trigger.keyval(), gtk::gdk::Key::Return);
    assert!(
        trigger.modifiers().is_empty(),
        "Shift+Return stays with TextView"
    );
    assert!(
        shortcut
            .action()
            .unwrap()
            .activate(gtk::ShortcutActionFlags::empty(), &editor, None)
    );
    wait_until(&context, || {
        chat.messages_after(0).len() == 2 && editor.buffer().char_count() == 0
    });
    assert_eq!(chat.messages_after(0).len(), 2);
    assert_eq!(editor.buffer().char_count(), 0);
    let rows: gtk::ListView = descendant(transcript.widget());
    let messages = rows.model().unwrap();
    let original = messages.item(0).unwrap();
    context.block_on(chat.send("second message".into()));
    drain(&context);
    assert_eq!(messages.n_items(), 4);
    assert_eq!(messages.item(0).unwrap(), original);
    let history: gtk::ListView = descendant(conversations.widget());
    let history_model = history.model().unwrap();
    let original_conversation = history_model.item(0).unwrap();
    let second = session.new_chat();
    context.block_on(second.send("another conversation".into()));
    drain(&context);
    assert_eq!(history_model.item(1).unwrap(), original_conversation);
    let search: gtk::SearchEntry = descendant(conversations.widget());
    search.set_text("another");
    search.emit_by_name::<()>("search-changed", &[]);
    drain(&context);
    assert_eq!(history_model.n_items(), 1);
    search.set_text("");
    search.emit_by_name::<()>("search-changed", &[]);
    drain(&context);
    let scroll: gtk::ScrolledWindow = descendant(composer.widget());
    editor.buffer().set_text(
        &(0..12)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    drain(&context);
    let metrics = editor.pango_context().metrics(None, None);
    let line = ((metrics.ascent() + metrics.descent()) / gtk::pango::SCALE).max(1);
    assert_eq!(scroll.max_content_height(), line * 7);
    // UI keystrokes remain visible while all Rust replacements are serialized.
    for index in 0..100 {
        editor.buffer().set_text(&format!("edit {index}"));
    }
    assert_eq!(
        editor.buffer().text(
            &editor.buffer().start_iter(),
            &editor.buffer().end_iter(),
            true
        ),
        "edit 99"
    );
    editor.activate_action("composer.send", None).unwrap();
    wait_until(&context, || {
        chat.messages_after(0).len() == 6 && editor.buffer().char_count() == 0
    });
    assert_eq!(chat.messages_after(0)[4].text, "edit 99");
    context.block_on(second.composer().replace("other draft".into()));
    context.block_on(chat.composer().replace("updated draft".into()));
    drain(&context);
    let buffer = editor.buffer();
    assert_eq!(
        buffer.text(&buffer.start_iter(), &buffer.end_iter(), true),
        "updated draft"
    );
    assert_eq!(second.composer().state().text, "other draft");
    content.remove(composer.widget());
    drop(composer);
    drain(&context);
    context.block_on(chat.composer().replace("after unmount".into()));
    drain(&context);
    assert_eq!(
        buffer.text(&buffer.start_iter(), &buffer.end_iter(), true),
        "updated draft",
        "unmounted composer must stop watching"
    );
    window.close();
    let shell = crate::shell::Shell::builder()
        .launch(session.clone())
        .detach();
    shell.widget().present();
    drain(&context);
    shell.emit(crate::shell::Msg::Select(chat.id().unwrap()));
    drain(&context);
    let draft: gtk::TextView = descendant(shell.widget());
    draft.buffer().set_text("preserved immediately");
    shell.emit(crate::shell::Msg::Select(second.id().unwrap()));
    drain(&context);
    shell.emit(crate::shell::Msg::Select(chat.id().unwrap()));
    drain(&context);
    let restored: gtk::TextView = descendant(shell.widget());
    assert_eq!(
        restored.buffer(),
        draft.buffer(),
        "switching keeps the native buffer and undo history"
    );
    assert_eq!(
        restored.buffer().text(
            &restored.buffer().start_iter(),
            &restored.buffer().end_iter(),
            true
        ),
        "preserved immediately"
    );
    WidgetExt::activate_action(shell.widget(), "win.search", None).unwrap();
    drain(&context);
    let search: gtk::SearchEntry = descendant(shell.widget());
    search.set_text("filter");
    WidgetExt::activate_action(shell.widget(), "win.escape", None).unwrap();
    drain(&context);
    assert!(search.text().is_empty());
    shell.widget().close();
    drop(shell);
    drain(&context);
}
