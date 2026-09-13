//! Native Linux UI built with relm4 over GTK4, without libadwaita.
//!
//! The root owns the desktop executor and ChildHost and binds feature clients to IPC.
//! The local observation module refreshes GObject properties on GLib and binds
//! them to widgets. Each watch owns one native GLib task, internally a task source
//! plus gtk-rs's child waker source, with no timer or per-invalidation task.
//! A keyed gio::ListModel fetches only transcript additions; GtkListView recycles
//! message and conversation widgets. The sidebar filters its model with SearchEntry.
//! Decoration policy lives in decorations.rs. An unmapped ordinary GTK window
//! reveals GTK's compositor preference without forcing client decorations.
//! Server frames win; portal button-layout then gtk-decoration-layout determine
//! client button positions. Tilers ignore compatibility GNOME portal defaults.
//! Hyprland/Sway/river/niri and empty layouts use a real toolbar with no window
//! controls. KDE uses compositor frames when offered and never gets an invented
//! GNOME or Breeze layout. Only GNOME has a GNOME fallback. Settings and toplevel
//! changes re-evaluate the policy. ShortcutController exposes gio actions.
//! The split collapses below 720 logical pixels; the reading column caps at 880.
//! Composer buffers and ordered command consumers survive conversation switches,
//! as do transcript reading positions. Only navigation is persisted here, under
//! XDG_STATE_HOME/arut/linux-ui; the node owns durable conversations and drafts.
//! Chat and composer errors are typed enums from the core (ADR 0016); strings.rs
//! maps every `ChatError`/`ComposerError`/`NodeFailure` variant, plus the typed
//! availability and connect-time RPC statuses, to a message id in the shared
//! Fluent source and asks `arut_i18n` for the sentence (ADR 0022). The language
//! list comes from GLib, so this surface follows the desktop's own setting.
//! GTK reads scheme and contrast from its portal settings. Theme colors come
//! from named GTK colors; ashpd supplies live accent and reduced-motion settings.
//! Only Arut classes receive additional CSS, and icons come from the desktop.
//! `mise run review:linux` uses a separate app identity, data directory, workspace,
//! and headless Hyprland output. ARUT_REVIEW=1 enables fixture actions only for
//! this run; real node messages and native TextBuffers exercise the normal UI.
//! The error fixture injects SessionError::Unavailable through Fluent accessors.
//! Non-Linux builds have no UI dependencies and run an empty main.

#[cfg(target_os = "linux")]
mod availability;
#[cfg(target_os = "linux")]
mod composer;
#[cfg(target_os = "linux")]
mod conversation_model;
#[cfg(target_os = "linux")]
mod conversations;
#[cfg(target_os = "linux")]
mod decorations;
#[cfg(target_os = "linux")]
mod layout;
#[cfg(target_os = "linux")]
mod message_model;
#[cfg(target_os = "linux")]
mod navigation;
#[cfg(target_os = "linux")]
mod observe;
#[cfg(target_os = "linux")]
mod review;
#[cfg(target_os = "linux")]
mod shell;
#[cfg(target_os = "linux")]
mod strings;
#[cfg(target_os = "linux")]
mod theme;
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
    let channel = match runtime
        .block_on(host.connect())
        .map_err(arut_product_session::SessionError::from)
    {
        Ok(channel) => channel,
        Err(error) => {
            eprintln!("{}", strings::rpc(&error));
            std::process::exit(1);
        }
    };
    eprintln!("arut-linux: arutd ready over IPC");
    let session = Rc::new(ProductSession::remote(
        channel,
        arut_product_session::SessionScope {
            node_id: "local".into(),
            workspace_id: "default".into(),
            pending_scope_id: "desktop".into(),
        },
        Arc::new(arut_runtime_local::NativeIds),
    ));
    // A separate application identity lets smoke tests use the real desktop
    // portal without activating an already-running personal instance.
    let app_id = std::env::var("ARUT_LINUX_APP_ID").unwrap_or_else(|_| "dev.arut.Arut".into());
    let app = relm4::RelmApp::new(&app_id);
    gtk::Window::set_default_icon_name("dev.arut.Arut");
    app.run::<shell::Shell>(session);
}

#[cfg(all(test, target_os = "linux"))]
mod component_tests;
