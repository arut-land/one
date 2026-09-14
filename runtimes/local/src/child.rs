//! Child lifetime follows the last channel. Readiness is an explicit handshake.
//!
//! `connect` asks whatever already answers at the stable per-user socket
//! (ADR 0011) before spawning anything: a live `arutd` there answers a
//! capability request and this reuses its channel; nothing answering deletes
//! the socket and spawns fresh, so two hosts never end up fighting over the
//! same node lease. A daemon this call does spawn gets a stdin pipe as well as
//! the stdout one the READY handshake already used, and this keeps its write
//! end open for exactly as long as the returned channel lives: closing it,
//! whether by `Drop` or because this process was killed and the OS closed
//! every file descriptor for it, is how `arutd` notices its parent is gone
//! even when nothing here ran to tell it so (`src/main.rs` also asks Linux for
//! `SIGTERM` on the same event through `PR_SET_PDEATHSIG`). A daemon that
//! cannot start at all says why through [`Readiness`] rather than surfacing a
//! raw I/O error.
//!
//! The kill half is `process-wrap`: the daemon runs in its own process group on
//! Unix and its own job object on Windows, so dropping the channel kills
//! whatever the daemon itself spawned rather than leaving it orphaned. The
//! reuse probe, the readiness handshake and the typed [`Readiness`] failures
//! above are ours; only the killing is not.
use crate::hosting::Scheduled;
use crate::readiness::Readiness;
use arut_product_session::hosting::{Host, HostMode};
use arut_protocol::capability::v1::{CapabilityServiceClient, GetCapabilitiesRequest};
use arut_rpc::{Request, RpcChannel, RpcFuture, Spawner, Status, StatusDetail, Wrap, Wrapped};
#[cfg(windows)]
use process_wrap::tokio::JobObject;
#[cfg(unix)]
use process_wrap::tokio::ProcessGroup;
use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use std::{path::Path, path::PathBuf, sync::Arc, time::Duration};
use tokio::io::BufReader;

/// How long a reuse probe waits for a capability answer before treating the
/// socket as unowned and spawning instead.
const PROBE_TIMEOUT: Duration = Duration::from_millis(500);

/// How long a spawned daemon has to print its handshake line before this gives
/// up on it (ROADMAP open decision "Daemon startup timeout"). Long enough for a
/// cold redb open on a slow disk, short enough that a surface does not appear
/// to hang. `ARUT_READY_TIMEOUT_MS` overrides it for a test or a slow host.
const READY_TIMEOUT: Duration = Duration::from_secs(10);

fn ready_timeout() -> Duration {
    std::env::var("ARUT_READY_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .map_or(READY_TIMEOUT, Duration::from_millis)
}

pub struct ChildHost {
    pub executable: PathBuf,
    pub socket: PathBuf,
    pub data: PathBuf,
    pub spawner: Arc<dyn Spawner>,
}

/// The stable per-user socket path (ADR 0011): the same path every launch
/// finds, rather than one keyed to the surface process's own pid, so a second
/// launch can find and reuse the first launch's daemon instead of racing it
/// for the node lease. Falls back to the temp directory with the uid in the
/// name when there is no `XDG_RUNTIME_DIR`, so it still cannot collide across
/// accounts on a shared machine.
#[must_use]
pub fn default_socket_path() -> PathBuf {
    if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime_dir).join("arut").join("node.sock");
    }
    let uid = rustix::process::getuid().as_raw();
    std::env::temp_dir().join(format!("arut-{uid}.sock"))
}

/// Schedules calls like any other host channel, and additionally owns the
/// daemon this call spawned.
struct ChildLifetime {
    scheduled: Scheduled,
    owned: Option<Owned>,
}
impl Wrap for ChildLifetime {
    fn wrap<T: Send + 'static>(
        &self,
        call: Box<dyn FnOnce() -> RpcFuture<T> + Send>,
    ) -> RpcFuture<T> {
        self.scheduled.wrap(call)
    }
}

/// Present only when this call spawned the daemon; a reused channel owns
/// nothing, so dropping it neither kills a daemon other callers may still be
/// using nor deletes the socket that names it.
struct Owned {
    /// Killing this kills the daemon's whole process group on Unix and its job
    /// object on Windows, so a helper the daemon spawned cannot outlive it.
    child: Box<dyn ChildWrapper>,
    // Never read. Its only job is staying alive for as long as the channel
    // does, keeping the pipe's write end open so `arutd` sees EOF exactly when
    // that ends, `Drop` or an outside kill alike.
    _stdin: tokio::process::ChildStdin,
    socket: PathBuf,
}
impl Drop for ChildLifetime {
    fn drop(&mut self) {
        let Some(owned) = &mut self.owned else {
            return;
        };
        let _ = owned.child.start_kill();
        let _ = std::fs::remove_file(&owned.socket);
    }
}
impl Host for ChildHost {
    fn mode(&self) -> HostMode {
        HostMode::ChildProcess
    }
    fn connect(&self) -> RpcFuture<Arc<dyn RpcChannel>> {
        let executable = self.executable.clone();
        let socket = self.socket.clone();
        let data = self.data.clone();
        let spawner = self.spawner.clone();
        Box::pin(async move {
            if let Some(channel) = reuse(&socket, &spawner).await {
                return Ok(channel);
            }
            // Either nothing was there or it did not answer; either way this
            // path does not get to keep whatever is at `socket`.
            let _ = std::fs::remove_file(&socket);
            spawn(executable, socket, data, spawner).await
        })
    }
}

/// Ask whatever is already at `socket` whether it is a working node.
///
/// `None` covers a missing socket, a refused connection, and one that accepts
/// a connection but never answers alike, so the caller's only decision is
/// "reuse this" or "clean up and spawn."
async fn reuse(socket: &Path, spawner: &Arc<dyn Spawner>) -> Option<Arc<dyn RpcChannel>> {
    if !socket.exists() {
        return None;
    }
    let transport = arut_transport::ipc::unix_socket(socket).ok()?;
    let channel: Arc<dyn RpcChannel> = Arc::new(transport);
    let client = CapabilityServiceClient::remote(channel.clone());
    let answered = tokio::time::timeout(
        PROBE_TIMEOUT,
        client.get_capabilities(Request::new(GetCapabilitiesRequest {})),
    )
    .await;
    match answered {
        Ok(Ok(_)) => Some(Arc::new(Wrapped::new(
            channel,
            ChildLifetime {
                scheduled: Scheduled(spawner.clone()),
                owned: None,
            },
        ))),
        _ => None,
    }
}

/// Spawn a fresh `arutd` and wait for its readiness handshake, translating a
/// daemon that could not start into the specific [`Readiness`] it reported
/// rather than letting its raw exit or I/O error through.
async fn spawn(
    executable: PathBuf,
    socket: PathBuf,
    data: PathBuf,
    spawner: Arc<dyn Spawner>,
) -> Result<Arc<dyn RpcChannel>, Status> {
    let mut command = CommandWrap::with_new(executable, |command| {
        command
            .env("ARUT_SOCKET", &socket)
            .env("ARUT_DATA", data)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped());
    });
    // The daemon owns whatever it spawns, so the kill has to reach the group,
    // not the one pid: a process group on Unix, a job object on Windows, which
    // is the half `command-group`'s successor exists to carry.
    #[cfg(unix)]
    command.wrap(ProcessGroup::leader());
    #[cfg(windows)]
    command.wrap(JobObject);
    command.wrap(KillOnDrop);
    let mut child = command
        .spawn()
        .map_err(|error| Readiness::SpawnFailed.into_status(error.to_string()))?;
    let stdin = child.stdin().take().expect("piped child stdin");
    let stdout = child.stdout().take().expect("piped child stdout");
    let read = tokio::time::timeout(ready_timeout(), async move {
        use prost::Message;
        use tokio::io::AsyncReadExt;

        let mut stdout = BufReader::new(stdout);
        let length = stdout.read_u32().await?;
        if length > 64 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "oversized readiness detail",
            ));
        }
        let mut payload = vec![0; length as usize];
        stdout.read_exact(&mut payload).await?;
        arut_protocol::runtime::local::v1::ReadinessDetail::decode(payload.as_slice())
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    })
    .await;
    match read {
        Err(_) => {
            return Err(Readiness::TimedOut
                .into_status("the daemon did not report readiness before the startup timeout"));
        }
        Ok(Err(error)) => return Err(Readiness::SpawnFailed.into_status(error.to_string())),
        Ok(Ok(_)) => {}
    }
    let state = read
        .expect("handled timeout")
        .expect("handled I/O failure")
        .try_into()
        .unwrap_or(Readiness::SpawnFailed);
    if state != Readiness::Ready {
        return Err(state.into_status(format!("the daemon exited before serving: {state}")));
    }
    let channel = arut_transport::ipc::unix_socket(&socket)
        .map_err(|error| Readiness::SocketUnreachable.into_status(error.to_string()))?;
    Ok(Arc::new(Wrapped::new(
        Arc::new(channel),
        ChildLifetime {
            scheduled: Scheduled(spawner),
            owned: Some(Owned {
                child,
                _stdin: stdin,
                socket,
            }),
        },
    )) as Arc<dyn RpcChannel>)
}
