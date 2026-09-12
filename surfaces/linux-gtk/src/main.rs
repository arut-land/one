//! Native Linux UI built with relm4 over GTK4, without libadwaita.
//!
//! The composition root owns the desktop executor and ChildHost. Components read
//! ProductSession handles directly and await independent watches on GLib. Only
//! navigation is persisted here, under XDG_STATE_HOME/arut/linux-ui.
//! Chat and composer errors are currently strings in the core; strings.rs maps
//! their typed statuses and the typed availability/RPC errors to English.
//! Non-Linux builds have no UI dependencies and run an empty main.

#[cfg(target_os = "linux")]
mod availability;
#[cfg(target_os = "linux")]
mod composer;
#[cfg(target_os = "linux")]
mod conversations;
#[cfg(target_os = "linux")]
mod navigation;
#[cfg(target_os = "linux")]
mod observe;
#[cfg(target_os = "linux")]
mod shell;
#[cfg(target_os = "linux")]
mod strings;
#[cfg(target_os = "linux")]
mod transcript;

#[cfg(not(target_os = "linux"))]
fn main() {}

#[cfg(target_os = "linux")]
fn main() {
    use arut_product_session::{ProductSession, hosting::Host};
    use arut_runtime_local::{
        child::ChildHost,
        hosting::{TokioSpawner, desktop_executor},
    };
    use std::{path::PathBuf, rc::Rc, sync::Arc};

    let runtime = desktop_executor().expect("desktop executor");
    let _entered = runtime.enter();
    let data = std::env::var_os("XDG_DATA_HOME")
        .map_or_else(
            || {
                PathBuf::from(std::env::var_os("HOME").expect("home directory"))
                    .join(".local/share")
            },
            PathBuf::from,
        )
        .join("arut");
    let host = ChildHost {
        executable: std::env::current_exe()
            .expect("executable path")
            .with_file_name("arutd"),
        socket: std::env::temp_dir().join(format!("arut-{}.sock", std::process::id())),
        data,
        spawner: Arc::new(TokioSpawner(runtime.handle().clone())),
    };
    let channel = match runtime.block_on(host.connect()) {
        Ok(channel) => channel,
        Err(error) => {
            eprintln!("{}", strings::rpc(&error));
            std::process::exit(1);
        }
    };
    eprintln!("arut-linux-gtk: arutd ready over IPC");
    let session = Rc::new(ProductSession::remote(
        channel,
        "desktop",
        Arc::new(arut_feature_chat::ports::NativeIds),
    ));
    let app = relm4::RelmApp::new("dev.arut.Arut");
    gtk::Window::set_default_icon_name("dev.arut.Arut");
    app.run::<shell::Shell>(session);
}
