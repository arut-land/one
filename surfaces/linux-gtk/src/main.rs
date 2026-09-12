//! # Linux GTK
//!
//! A Rust-owned GTK4 surface with conversation navigation, a transcript, a pending
//! composer, per-conversation drafts, and send. It reads Rust projections directly
//! and awaits watch changes on GLib's main context. It uses no FFI or libadwaita.

use arut_product_session::ProductSession;
use gtk::{glib, prelude::*};
use std::rc::Rc;

fn main() -> glib::ExitCode {
    let app = gtk::Application::builder()
        .application_id("dev.arut.Arut")
        .build();
    let runtime = std::sync::Arc::new(
        arut_runtime_local::hosting::desktop_executor().expect("desktop executor"),
    );
    app.connect_activate(move |app| {
        use arut_product_session::hosting::Host;
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("Arut")
            .default_width(900)
            .default_height(600)
            .build();
        window.set_child(Some(&gtk::Label::new(Some("Starting local node…"))));
        window.present();
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(std::env::var_os("HOME").expect("home directory"))
                    .join(".local/share")
            })
            .join("arut");
        let host = arut_runtime_local::child::ChildHost {
            executable: std::env::current_exe()
                .expect("executable path")
                .with_file_name("arutd"),
            socket: std::env::temp_dir().join(format!("arut-{}.sock", std::process::id())),
            data,
            spawner: std::sync::Arc::new(arut_runtime_local::hosting::TokioSpawner(
                runtime.handle().clone(),
            )),
        };
        let started = runtime.spawn(async move { host.connect().await });
        glib::spawn_future_local(async move {
            match started.await {
                Ok(Ok(channel)) => show_session(
                    &window,
                    Rc::new(ProductSession::remote(
                        channel,
                        "desktop",
                        std::sync::Arc::new(arut_feature_chat::ports::NativeIds),
                    )),
                ),
                _ => window.set_child(Some(&gtk::Label::new(Some(
                    "Could not start the local node.",
                )))),
            }
        });
    });
    app.run()
}

fn show_session(window: &gtk::ApplicationWindow, session: Rc<ProductSession>) {
    let initialize = session.clone();
    glib::spawn_future_local(async move {
        let _ = initialize.initialize().await;
    });
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let history = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let new = gtk::Button::with_label("New conversation");
    sidebar.append(&new);
    sidebar.append(&history);
    root.append(&sidebar);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
    root.append(&body);
    show_chat(&body, session.chat());
    let session_new = session.clone();
    let body_new = body.clone();
    new.connect_clicked(move |_| show_chat(&body_new, session_new.new_chat()));
    let changes = session.conversations_changes();
    let weak_history = history.downgrade();
    let weak_body = body.downgrade();
    glib::spawn_future_local(async move {
        while changes.changed().await.is_some() {
            let (Some(history), Some(body)) = (weak_history.upgrade(), weak_body.upgrade()) else {
                break;
            };
            while let Some(child) = history.first_child() {
                history.remove(&child);
            }
            for summary in session.chat_summaries() {
                let button = gtk::Button::with_label(&summary.title);
                let session = session.clone();
                let body = body.clone();
                button.connect_clicked(move |_| {
                    if let Some(chat) = session.select_chat(&summary.id) {
                        show_chat(&body, chat);
                    }
                });
                history.append(&button);
            }
        }
    });
    window.set_child(Some(&root));
}

fn show_chat(body: &gtk::Box, chat: arut_feature_chat::product::ChatClient) {
    while let Some(child) = body.first_child() {
        body.remove(&child);
    }
    let transcript = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hexpand(true)
        .child(&transcript)
        .build();
    body.append(&scroll);
    let entry = gtk::Entry::new();
    let composer = chat.composer();
    entry.set_text(&composer.state().text);
    body.append(&entry);
    let send = gtk::Button::with_label("Send");
    body.append(&send);
    let composer_edit = composer.clone();
    entry.connect_changed(move |entry| {
        let text = entry.text().to_string();
        if text == composer_edit.state().text {
            return;
        }
        let composer = composer_edit.clone();
        glib::spawn_future_local(async move {
            composer.replace(text).await;
        });
    });
    let chat_send = chat.clone();
    let weak_entry = entry.downgrade();
    send.connect_clicked(move |_| {
        if let Some(entry) = weak_entry.upgrade() {
            let text = entry.text().to_string();
            let chat = chat_send.clone();
            glib::spawn_future_local(async move {
                chat.send(text).await;
            });
        }
    });
    let changes = chat.changes();
    let weak_transcript = transcript.downgrade();
    glib::spawn_future_local(async move {
        while changes.changed().await.is_some() {
            let Some(transcript) = weak_transcript.upgrade() else {
                break;
            };
            while let Some(child) = transcript.first_child() {
                transcript.remove(&child);
            }
            for message in chat.state().messages {
                let label = gtk::Label::new(Some(&message.text));
                label.set_wrap(true);
                label.set_xalign(0.0);
                transcript.append(&label);
            }
        }
    });
    let follow_composer = composer.clone();
    let follow = glib::spawn_future_local(async move {
        follow_composer.follow().await;
    });
    entry.connect_destroy(move |_| follow.abort());
    let changes = composer.changes();
    let weak_entry = entry.downgrade();
    glib::spawn_future_local(async move {
        composer.initialize().await;
        while changes.changed().await.is_some() {
            let Some(entry) = weak_entry.upgrade() else {
                break;
            };
            let text = composer.state().text;
            if entry.text().as_str() != text {
                entry.set_text(&text);
            }
        }
    });
}
