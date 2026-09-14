//! Daemon composition root: choose native ports, list features, serve their routers.
//!
//! Dying with the parent that spawned it is part of this binary's contract
//! (ADR 0011): an orphaned `arutd` keeps the exclusive lock on `node.lock`
//! forever, so the next launch cannot start a node at all. Two independent
//! signals cover it: `ChildHost` keeps this process's stdin pipe open for as
//! long as it wants the daemon alive, so its EOF -- which the OS delivers
//! whether the parent closed it on purpose or was killed without a chance to
//! -- means time to go, on every platform a pipe works on; Linux additionally
//! asks for `SIGTERM` on the same event through `PR_SET_PDEATHSIG`, which
//! macOS has no equivalent for. Either path removes the socket before
//! exiting; the node lease needs no code of its own; closing every file
//! descriptor, which is how a process ends whichever way it ends, releases it.
use arut_product_session::feature::{Chat, ComposeSet, Services};
use arut_runtime_local::readiness::Readiness;
use std::{env, path::PathBuf};

/// What this daemon serves. One line per feature; the manifest, the routers and
/// the session's clients all follow from it (ADR 0025).
type Features = (Chat,);

/// The handshake `ChildHost` reads on this process's stdout; one table names
/// both ends (`arut_runtime_local::readiness`).
fn report(state: Readiness) {
    println!("{}", state.line());
}

/// Best-effort: ask Linux to send `SIGTERM` when our parent dies, so an
/// abrupt parent kill does not leave this process behind as an orphan. A
/// failure here (the call is not available, or the kernel refuses it) leaves
/// the stdin watchdog below as the only signal, which is enough on its own.
///
/// The setting is per-thread, so `main` asks for it before it builds the Tokio
/// runtime: the thread that holds it is then the one that outlives every
/// worker, and no worker thread starts without it.
#[cfg(target_os = "linux")]
fn die_with_parent() {
    let _ = rustix::process::set_parent_process_death_signal(Some(rustix::process::Signal::TERM));
}
#[cfg(not(target_os = "linux"))]
fn die_with_parent() {}

/// Remove the socket, if we ever bound one, and exit. Every other resource
/// this process holds, the node lease included, is released by the OS
/// closing our file descriptors as part of the process ending, so nothing
/// else here needs an explicit release.
fn shut_down(socket: Option<&PathBuf>) -> ! {
    if let Some(socket) = socket {
        let _ = std::fs::remove_file(socket);
    }
    std::process::exit(0)
}

/// `ChildHost` pipes this process's stdin and holds the write end open for
/// exactly as long as it wants us alive; reading EOF here means that end
/// closed, whether `ChildHost` dropped it on purpose or the process holding it
/// was killed and the OS closed it for us. Works wherever a pipe does,
/// `PR_SET_PDEATHSIG` above being Linux-only.
fn watch_parent_stdin(socket: Option<PathBuf>) {
    tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut stdin = tokio::io::stdin();
        let mut byte = [0u8; 1];
        loop {
            match stdin.read(&mut byte).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
        shut_down(socket.as_ref());
    });
}

/// Catch `SIGTERM` -- delivered by `PR_SET_PDEATHSIG` above, or by whatever
/// else sends it -- long enough to remove the socket before exiting; the
/// default disposition would exit without running that cleanup.
#[cfg(unix)]
fn watch_terminate(socket: Option<PathBuf>) -> std::io::Result<()> {
    let mut signal = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::spawn(async move {
        signal.recv().await;
        shut_down(socket.as_ref());
    });
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    die_with_parent();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(serve())
}

async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let socket = env::var("ARUT_SOCKET").ok().map(PathBuf::from);
    watch_parent_stdin(socket.clone());
    #[cfg(unix)]
    watch_terminate(socket.clone())?;

    let data = PathBuf::from(env::var("ARUT_DATA").unwrap_or_else(|_| "arut-chat.pb".into()));
    let runtime = match arut_runtime_local::LocalRuntime::open(data, "chat") {
        Ok(runtime) => std::sync::Arc::new(runtime),
        // A held `try_lock` surfaces as exactly this `io::ErrorKind` (ADR
        // 0011): another node is already running against this data directory.
        Err(arut_storage::StorageError::Io(std::io::ErrorKind::WouldBlock)) => {
            report(Readiness::LeaseHeld);
            return Err(Box::<dyn std::error::Error>::from(Readiness::LeaseHeld));
        }
        Err(error) => {
            report(Readiness::SpawnFailed);
            return Err(Box::<dyn std::error::Error>::from(error));
        }
    };
    let composed = <Features as ComposeSet<_>>::compose(&runtime)?;
    let app = arut_runtime_local::Node::serve::<_, Services<Features>>(runtime, composed.routers)?;
    #[cfg(unix)]
    if let Some(socket) = &socket {
        use std::os::unix::fs::PermissionsExt;
        if let Some(parent) = socket.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let listener = match tokio::net::UnixListener::bind(socket) {
            Ok(listener) => listener,
            Err(error) => {
                report(Readiness::SocketUnreachable);
                return Err(Box::<dyn std::error::Error>::from(error));
            }
        };
        std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
        report(Readiness::Ready);
        axum::serve(listener, app).await?;
        let _ = std::fs::remove_file(socket);
        return Ok(());
    }
    let address = env::var("ARUT_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let listener = tokio::net::TcpListener::bind(&address).await?;
    report(Readiness::Ready);
    axum::serve(listener, app).await?;
    Ok(())
}
