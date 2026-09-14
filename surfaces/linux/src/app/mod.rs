//! Linux application and its unconditional native module tree.
mod composer;
mod conversation_model;
mod conversations;
mod decorations;
mod icons;
mod layout;
mod motion;
mod navigation;
#[cfg(feature = "review")]
mod review;
mod shell;
mod strings;
mod theme;
mod transcript;

#[cfg(test)]
mod component_tests;
#[cfg(any(test, feature = "review"))]
mod testing;

/// The features this surface composes. Naming the set once here is what lets
/// every other module write `Rc<Session>` without restating it.
pub type Session = arut_product_session::ProductSession<(arut_product_session::feature::Chat,)>;

pub fn run() {
    use arut_product_session::hosting::Host;
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
        socket: arut_runtime_local::child::default_socket_path(),
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
    let session: Rc<Session> = Rc::new(Session::remote(
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
    icons::install(&gtk::gdk::Display::default().expect("display"));
    gtk::Window::set_default_icon_name("dev.arut.Arut");
    app.run::<shell::Shell>(session);
}
