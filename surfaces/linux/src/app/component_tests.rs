//! Display-backed checks of the watch/component boundary. No screenshots.
//!
//! Each test runs in its own child process so GTK's process-wide policy and the
//! isolated `XDG_*` directories are set before GTK loads. Run them with
//! `cargo test -p arut-linux -- --ignored --test-threads=1`.
use crate::app::Session;
use crate::app::{
    composer::{Composer, Msg as ComposerMsg},
    conversation_model::ConversationItem,
    shell::Shell,
    testing::{descendant, find},
    transcript::{Msg as TranscriptMsg, Transcript},
};
use crate::glib_observe::Tasks;
use arut_product_session::{SessionScope, chat::ChatClient, hosting::Host};
use arut_runtime_local::{
    child::ChildHost,
    hosting::{TokioSpawner, desktop_executor},
};
use gtk::{gio::prelude::ActionGroupExt, glib, prelude::*};
use relm4::{Component, ComponentController};
use std::{cell::Cell, path::PathBuf, rc::Rc, sync::Arc};

struct Fixture {
    context: glib::MainContext,
    session: Rc<Session>,
    directory: PathBuf,
    _app: relm4::RelmApp<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// `None` means this process was the parent: it has already run `name` in an
/// isolated child and asserted that it passed, so the caller returns.
fn fixture(name: &str) -> Option<Fixture> {
    if std::env::var_os("ARUT_GTK_TEST_CHILD").is_none() {
        run_isolated(name);
        return None;
    }
    glib::log_set_always_fatal(glib::LogLevels::LEVEL_ERROR | glib::LogLevels::LEVEL_CRITICAL);
    let app = relm4::RelmApp::<()>::new("dev.arut.ComponentTest");
    let context = glib::MainContext::default();
    std::mem::forget(context.acquire().unwrap());
    let executor: &'static _ = Box::leak(Box::new(desktop_executor().unwrap()));
    std::mem::forget(executor.enter());
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
    let session: Rc<Session> = Rc::new(Session::remote(
        channel,
        SessionScope {
            node_id: "local".into(),
            workspace_id: "default".into(),
            pending_scope_id: "component-test".into(),
        },
        Arc::new(arut_runtime_local::NativeIds),
    ));
    context.block_on(session.refresh_capabilities());
    Some(Fixture {
        context,
        session,
        directory,
        _app: app,
    })
}

fn run_isolated(name: &str) {
    // Short: the node's Unix socket path is built under this directory and the
    // platform caps it at 108 bytes.
    let directory = std::env::temp_dir().join(format!("arut-gtk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir(&directory).unwrap();
    let path = format!("{}::{name}", module_path!().split_once("::").unwrap().1);
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            &path,
            "--ignored",
            "--test-threads=1",
            "--nocapture",
        ])
        .env("ARUT_GTK_TEST_CHILD", "1")
        .env("G_DEBUG", "fatal-criticals")
        .env("XDG_STATE_HOME", directory.join("state"))
        .env("XDG_DATA_HOME", directory.join("data"))
        .env("TMPDIR", &directory)
        .status();
    let _ = std::fs::remove_dir_all(&directory);
    assert!(result.unwrap().success(), "GTK component child failed");
}

impl Fixture {
    fn drain(&self) {
        let mut iterations = 0;
        while self.context.pending() {
            self.context.iteration(false);
            iterations += 1;
            assert!(iterations < 10_000, "UI did not become idle");
        }
    }

    fn wait_until(&self, ready: impl Fn() -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            self.drain();
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

    fn send(&self, chat: &ChatClient, text: &str) {
        self.context.block_on(chat.send(text.to_owned()));
        self.drain();
    }
}

/// Realizes and allocates without mapping: tests must not take desktop focus.
fn mount(children: &[&gtk::Widget]) -> gtk::Window {
    let window = gtk::Window::new();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    for child in children {
        content.append(*child);
    }
    window.set_child(Some(&content));
    WidgetExt::realize(&window);
    content.allocate(800, 600, -1, None);
    window
}

fn text_of(buffer: &gtk::TextBuffer) -> String {
    buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string()
}

#[test]
#[ignore = "requires a GTK display; run with --ignored --test-threads=1"]
fn watch_observers_coalesce_and_stop_on_drop() {
    let Some(fixture) = fixture("watch_observers_coalesce_and_stop_on_drop") else {
        return;
    };
    let composer = fixture.session.chat().composer();
    let reads = Rc::new(Cell::new(0_usize));
    let mut tasks = Tasks::default();
    tasks.observe(composer.changes(), {
        let reads = reads.clone();
        move || reads.set(reads.get() + 1)
    });
    fixture.drain();
    let initial = reads.get();
    // `replace` writes the draft as it is called; the returned flush is not
    // driven here, so this is a burst of revisions with no main-loop turn.
    for index in 0..1_000 {
        drop(composer.replace(format!("draft {index}")));
    }
    fixture.drain();
    assert_eq!(reads.get(), initial + 1, "a burst of revisions reads once");
    assert_eq!(composer.state().text, "draft 999");
    drop(tasks);
    drop(composer.replace("after cancellation".into()));
    fixture.drain();
    assert_eq!(
        reads.get(),
        initial + 1,
        "dropping the observer rejects queued updates"
    );
}

#[test]
#[ignore = "requires a GTK display; run with --ignored --test-threads=1"]
fn bundled_icons_resolve_without_an_installed_theme() {
    let Some(_fixture) = fixture("bundled_icons_resolve_without_an_installed_theme") else {
        return;
    };
    let display = gtk::gdk::Display::default().expect("display");
    crate::app::icons::install(&display);
    let theme = gtk::IconTheme::for_display(&display);
    for name in [
        "arut-sidebar-hide-symbolic",
        "arut-sidebar-show-symbolic",
        "arut-new-conversation-symbolic",
        "arut-menu-symbolic",
        "arut-send-symbolic",
        "arut-latest-symbolic",
        "arut-warning-symbolic",
        "arut-empty-symbolic",
        "arut-unread-symbolic",
        "dev.arut.Arut",
    ] {
        assert!(
            theme.has_icon(name),
            "{name} is not in the bundled resource"
        );
    }
    let icon = theme.lookup_icon(
        "arut-send-symbolic",
        &[],
        16,
        1,
        gtk::TextDirection::Ltr,
        gtk::IconLookupFlags::empty(),
    );
    assert_eq!(
        icon.file().map(|file| file.uri().to_string()).as_deref(),
        Some("resource:///dev/arut/Arut/icons/scalable/actions/arut-send-symbolic.svg"),
        "the icon comes from the bundled resource, under the symbolic name GTK recolors"
    );
}

#[test]
#[ignore = "requires a GTK display; run with --ignored --test-threads=1"]
fn column_caps_reading_width() {
    let Some(_fixture) = fixture("column_caps_reading_width") else {
        return;
    };
    let wrapped = gtk::Label::new(Some(
        &"A paragraph that must wrap when the sidebar opens. ".repeat(20),
    ));
    wrapped.set_wrap(true);
    let column = crate::app::layout::Column::new(&wrapped, 880);
    assert_eq!(column.request_mode(), gtk::SizeRequestMode::HeightForWidth);
    assert!(
        column.measure(gtk::Orientation::Vertical, 320).1
            > column.measure(gtk::Orientation::Vertical, 880).1,
        "a narrower column wraps to a taller one"
    );
    assert_eq!(
        column.measure(gtk::Orientation::Vertical, 1920).1,
        column.measure(gtk::Orientation::Vertical, 880).1,
        "past the cap the height stops changing"
    );
}

#[test]
#[ignore = "requires a GTK display; run with --ignored --test-threads=1"]
fn transcript_appends_without_rebuilding_rows() {
    let Some(fixture) = fixture("transcript_appends_without_rebuilding_rows") else {
        return;
    };
    let chat = fixture.session.chat();
    let transcript = Transcript::builder().launch(chat.clone()).detach();
    let window = mount(&[transcript.widget().clone().upcast_ref()]);
    let scroll: gtk::ScrolledWindow = descendant(transcript.widget()).unwrap();
    let latest: gtk::Revealer = descendant(transcript.widget()).unwrap();
    let viewport = scroll.height();
    latest.set_reveal_child(true);
    window.child().unwrap().allocate(800, 600, -1, None);
    assert_eq!(
        scroll.height(),
        viewport,
        "the latest-message overlay must not resize the transcript"
    );
    fixture.send(&chat, "first message");
    let rows: gtk::ListView = descendant(transcript.widget()).unwrap();
    let messages = rows.model().unwrap();
    assert_eq!(messages.n_items(), 2, "the echo node answers");
    let original = messages.item(0).unwrap();
    fixture.send(&chat, "second message");
    assert_eq!(messages.n_items(), 4);
    assert_eq!(
        messages.item(0).unwrap(),
        original,
        "rows are keyed, not rebuilt"
    );
}

#[test]
#[ignore = "requires a GTK display; run with --ignored --test-threads=1"]
fn transcript_swaps_conversations_without_a_new_component() {
    let Some(fixture) = fixture("transcript_swaps_conversations_without_a_new_component") else {
        return;
    };
    let first = fixture.session.new_chat();
    fixture.send(&first, "first conversation");
    let second = fixture.session.new_chat();
    fixture.send(&second, "second conversation");
    let transcript = Transcript::builder().launch(first.clone()).detach();
    mount(&[transcript.widget().clone().upcast_ref()]);
    let rows: gtk::ListView = descendant(transcript.widget()).unwrap();
    let messages = rows.model().unwrap();
    fixture.wait_until(|| messages.n_items() == 2);
    transcript.emit(TranscriptMsg::Chat(Box::new(second.clone())));
    fixture.drain();
    assert_eq!(messages.n_items(), 2, "the model is reset, not appended to");
    let shown = messages
        .item(0)
        .unwrap()
        .downcast::<glib::BoxedAnyObject>()
        .unwrap();
    assert_eq!(
        shown
            .borrow::<arut_product_session::chat::ChatMessage>()
            .text,
        "second conversation"
    );
}

#[test]
#[ignore = "requires a GTK display; run with --ignored --test-threads=1"]
fn composer_coalesces_edits_and_sends() {
    let Some(fixture) = fixture("composer_coalesces_edits_and_sends") else {
        return;
    };
    let chat = fixture.session.chat();
    let composer = Composer::builder().launch(chat.clone()).detach();
    mount(&[composer.widget().clone().upcast_ref()]);
    composer.emit(ComposerMsg::Enabled(true));
    fixture.drain();
    let editor: gtk::TextView = descendant(composer.widget()).unwrap();
    // Every keystroke stays visible while the node sees only the latest draft.
    for index in 0..100 {
        editor.buffer().set_text(&format!("edit {index}"));
    }
    assert_eq!(text_of(&editor.buffer()), "edit 99");
    fixture.wait_until(|| chat.composer().state().text == "edit 99");
    editor.activate_action("composer.send", None).unwrap();
    fixture.wait_until(|| chat.messages_after(0).len() == 2 && editor.buffer().char_count() == 0);
    assert_eq!(chat.messages_after(0)[0].text, "edit 99");
}

#[test]
#[ignore = "requires a GTK display; run with --ignored --test-threads=1"]
fn composer_sends_on_return_and_offers_no_send_for_a_blank_draft() {
    let Some(fixture) = fixture("composer_sends_on_return_and_offers_no_send_for_a_blank_draft")
    else {
        return;
    };
    let chat = fixture.session.chat();
    let composer = Composer::builder().launch(chat.clone()).detach();
    mount(&[composer.widget().clone().upcast_ref()]);
    composer.emit(ComposerMsg::Enabled(true));
    fixture.drain();
    let editor: gtk::TextView = descendant(composer.widget()).unwrap();
    let send: gtk::Button = descendant(composer.widget()).unwrap();
    editor.buffer().set_text("   \n  ");
    fixture.drain();
    assert!(!send.is_sensitive(), "whitespace is not a message");
    editor.buffer().set_text("typed with the keyboard");
    fixture.drain();
    assert!(send.is_sensitive());
    let shortcuts = descendant::<gtk::TextView>(composer.widget())
        .unwrap()
        .observe_controllers();
    let shortcuts = (0..shortcuts.n_items())
        .filter_map(|position| shortcuts.item(position))
        .filter_map(|object| object.downcast::<gtk::ShortcutController>().ok())
        .find(|controller| controller.n_items() == 4)
        .expect("composer shortcuts");
    let shortcut = shortcuts
        .item(0)
        .unwrap()
        .downcast::<gtk::Shortcut>()
        .unwrap();
    assert!(
        shortcut
            .action()
            .unwrap()
            .activate(gtk::ShortcutActionFlags::empty(), &editor, None)
    );
    fixture.wait_until(|| chat.messages_after(0).len() == 2 && editor.buffer().char_count() == 0);
    assert_eq!(chat.messages_after(0)[0].text, "typed with the keyboard");
}

#[test]
#[ignore = "requires a GTK display; run with --ignored --test-threads=1"]
fn composer_stops_observing_when_unmounted() {
    let Some(fixture) = fixture("composer_stops_observing_when_unmounted") else {
        return;
    };
    let chat = fixture.session.chat();
    let composer = Composer::builder().launch(chat.clone()).detach();
    let window = mount(&[composer.widget().clone().upcast_ref()]);
    let editor: gtk::TextView = descendant(composer.widget()).unwrap();
    let buffer = editor.buffer();
    fixture
        .context
        .block_on(chat.composer().replace("updated draft".into()));
    fixture.drain();
    assert_eq!(text_of(&buffer), "updated draft");
    window
        .child()
        .unwrap()
        .downcast::<gtk::Box>()
        .unwrap()
        .remove(composer.widget());
    drop(composer);
    fixture.drain();
    fixture
        .context
        .block_on(chat.composer().replace("after unmount".into()));
    fixture.drain();
    assert_eq!(
        text_of(&buffer),
        "updated draft",
        "an unmounted composer must stop watching"
    );
}

#[test]
#[ignore = "requires a GTK display; run with --ignored --test-threads=1"]
fn composer_sizes_to_seven_lines() {
    let Some(fixture) = fixture("composer_sizes_to_seven_lines") else {
        return;
    };
    let composer = Composer::builder().launch(fixture.session.chat()).detach();
    mount(&[composer.widget().clone().upcast_ref()]);
    let editor: gtk::TextView = descendant(composer.widget()).unwrap();
    let scroll: gtk::ScrolledWindow = descendant(composer.widget()).unwrap();
    editor.buffer().set_text(
        &(0..12)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    fixture.drain();
    // Seven complete lines must fit, including descent at fractional font sizes.
    let first = editor.iter_location(&editor.buffer().iter_at_line(0).unwrap());
    let seventh = editor.iter_location(&editor.buffer().iter_at_line(6).unwrap());
    let inset = editor.top_margin();
    assert!(scroll.max_content_height() >= seventh.y() + seventh.height() - first.y() + inset);
    let eighth = editor.iter_location(&editor.buffer().iter_at_line(7).unwrap());
    assert!(scroll.max_content_height() < eighth.y() + eighth.height() - first.y() + inset);
}

#[test]
#[ignore = "requires a GTK display; run with --ignored --test-threads=1"]
fn sidebar_filters_and_keeps_selection() {
    let Some(fixture) = fixture("sidebar_filters_and_keeps_selection") else {
        return;
    };
    let first = fixture.session.new_chat();
    fixture.send(&first, "Saturday by the river");
    let second = fixture.session.new_chat();
    fixture.send(&second, "another conversation");
    let conversations = crate::app::conversations::Conversations::builder()
        .launch(fixture.session.clone())
        .detach();
    mount(&[conversations.widget().clone().upcast_ref()]);
    fixture.session.select(first.id());
    fixture.drain();
    let list: gtk::ListView = descendant(conversations.widget()).unwrap();
    let selection = list
        .model()
        .unwrap()
        .downcast::<gtk::SingleSelection>()
        .unwrap();
    assert_eq!(selection.n_items(), 2);
    let selected = selection.selected_item().unwrap();
    let search: gtk::SearchEntry = descendant(conversations.widget()).unwrap();
    search.set_text("another");
    fixture.drain();
    assert_eq!(selection.n_items(), 1, "the search narrows the list");
    search.set_text("");
    fixture.drain();
    assert_eq!(
        selection.selected_item().unwrap(),
        selected,
        "clearing the search restores the selected conversation"
    );
    let row = selected.downcast::<ConversationItem>().unwrap();
    assert!(!row.preview().is_empty());
}

#[test]
#[ignore = "requires a GTK display; run with --ignored --test-threads=1"]
fn shell_restores_navigation_and_per_chat_drafts() {
    let Some(fixture) = fixture("shell_restores_navigation_and_per_chat_drafts") else {
        return;
    };
    let chat = fixture.session.new_chat();
    fixture.send(&chat, "restored conversation");
    let second = fixture.session.new_chat();
    fixture.send(&second, "another conversation");
    fixture
        .context
        .block_on(chat.composer().replace("saved draft".into()));
    crate::app::navigation::Navigation {
        selected: chat.id(),
        collapsed: false,
        ..Default::default()
    }
    .save();
    let shell = Shell::builder().launch(fixture.session.clone()).detach();
    WidgetExt::realize(shell.widget());
    fixture.wait_until(|| {
        descendant::<gtk::TextView>(shell.widget())
            .is_some_and(|editor| text_of(&editor.buffer()) == "saved draft")
    });
    let draft: gtk::TextView = descendant(shell.widget()).unwrap();
    draft.buffer().set_text("preserved immediately");
    fixture.session.select(second.id());
    fixture.drain();
    fixture.session.select(chat.id());
    fixture.drain();
    let restored: gtk::TextView = descendant(shell.widget()).unwrap();
    assert_eq!(
        restored.buffer(),
        draft.buffer(),
        "switching keeps the native buffer and undo history"
    );
    assert_eq!(text_of(&restored.buffer()), "preserved immediately");
    shell.widget().close();
}

#[test]
#[ignore = "requires a GTK display; run with --ignored --test-threads=1"]
fn shell_availability_and_search_follow_the_session() {
    let Some(fixture) = fixture("shell_availability_and_search_follow_the_session") else {
        return;
    };
    let shell = Shell::builder().launch(fixture.session.clone()).detach();
    WidgetExt::realize(shell.widget());
    let availability = find(shell.widget().upcast_ref(), &|widget| {
        widget.has_css_class("arut-availability")
    })
    .unwrap()
    .downcast::<gtk::Label>()
    .unwrap();
    fixture.wait_until(|| availability.label() == "Ready");
    WidgetExt::activate_action(shell.widget(), "win.search", None).unwrap();
    fixture.drain();
    let search: gtk::SearchEntry = descendant(shell.widget()).unwrap();
    search.set_text("filter");
    WidgetExt::activate_action(shell.widget(), "win.escape", None).unwrap();
    fixture.drain();
    assert!(search.text().is_empty(), "Escape clears an active search");
    let sidebar: gtk::Revealer = descendant(shell.widget()).unwrap();
    assert!(sidebar.reveals_child());
    WidgetExt::activate_action(shell.widget(), "win.sidebar", None).unwrap();
    fixture.drain();
    shell
        .widget()
        .child()
        .unwrap()
        .allocate(1000, 600, -1, None);
    assert!(!sidebar.reveals_child());
    let (minimum, natural, _, _) = sidebar.measure(gtk::Orientation::Horizontal, -1);
    assert_eq!(
        (minimum, natural),
        (0, 0),
        "a collapsed sidebar releases all width"
    );
    shell.widget().close();
}

#[test]
#[ignore = "requires a GTK display; run with --ignored --test-threads=1"]
// `GtkShortcutsWindow` is deprecated in GTK 4.18; see the contract note on
// `shell::install_overlay` for why this surface still uses it.
#[expect(deprecated)]
fn every_shortcut_reaches_an_action_the_menu_and_the_overlay_name() {
    let Some(fixture) = fixture("every_shortcut_reaches_an_action_the_menu_and_the_overlay_name")
    else {
        return;
    };
    let shell = Shell::builder().launch(fixture.session.clone()).detach();
    WidgetExt::realize(shell.widget());
    fixture.drain();
    let window = shell.widget();
    for action in [
        "new",
        "sidebar",
        "composer",
        "search",
        "escape",
        "latest",
        "about",
        "show-help-overlay",
    ] {
        assert!(
            ActionGroupExt::has_action(window, action),
            "win.{action} is missing"
        );
    }
    let controllers = window.observe_controllers();
    let shortcuts = (0..controllers.n_items())
        .filter_map(|position| controllers.item(position))
        .filter_map(|object| object.downcast::<gtk::ShortcutController>().ok())
        .flat_map(|controller| {
            (0..controller.n_items())
                .filter_map(move |position| controller.item(position))
                .filter_map(|object| object.downcast::<gtk::Shortcut>().ok())
        })
        .filter_map(|shortcut| shortcut.trigger())
        .map(|trigger| trigger.to_string())
        .collect::<Vec<_>>();
    for trigger in [
        "<Control>n",
        "<Control>b",
        "<Control>l",
        "<Control>k",
        "<Control>f",
        "Escape",
        "<Control>question",
    ] {
        assert!(
            shortcuts.iter().any(|registered| registered == trigger),
            "{trigger} is not registered; have {shortcuts:?}"
        );
    }
    let menu: gtk::MenuButton = descendant(window).expect("primary menu");
    assert_eq!(
        menu.menu_model().expect("a menu model").n_items(),
        3,
        "About, Keyboard Shortcuts and Quit"
    );
    let overlay = window.help_overlay().expect("a shortcuts window");
    assert!(
        descendant::<gtk::ShortcutsShortcut>(&overlay).is_some(),
        "the overlay is built from the shortcut table"
    );
    shell.widget().close();
}
