#![cfg(unix)]
//! `ChildHost` lifecycle: readiness, dying with the parent, and reuse over
//! spawning a second daemon against the same node lease (ADR 0011).
use arut_product_session::{ProductSession, hosting::Host};
use arut_rpc::StatusDetail;
use arut_runtime_host_polled::NativeIds;
use arut_runtime_local::{child::ChildHost, hosting::TokioSpawner, readiness::Readiness};
use std::sync::Arc;
use std::time::Duration;

/// A fresh, empty directory this process alone uses, torn down by the caller.
fn scratch_dir(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("arut-child-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn spawner() -> Arc<dyn arut_rpc::Spawner> {
    Arc::new(TokioSpawner(tokio::runtime::Handle::current()))
}

/// Sends one message over `channel` and proves it actually reached a working
/// node, rather than merely not having errored while connecting: the reply is
/// there and the conversation it started is listed under the text that started
/// it.
async fn assert_channel_works(channel: Arc<dyn arut_rpc::RpcChannel>, text: &str) {
    let session = ProductSession::remote(
        channel,
        arut_product_session::SessionScope {
            node_id: "local".into(),
            workspace_id: "default".into(),
            pending_scope_id: "test".into(),
        },
        Arc::new(NativeIds),
    );
    let chat = session.chat();
    let state = chat.send(text.into()).await;
    assert_eq!(chat.messages_after(0).len(), 2, "{:?}", state.error);
    assert!(
        session
            .chat_summaries()
            .iter()
            .any(|summary| summary.title == text)
    );
}

/// The abrupt-parent-death case (ADR 0011): nothing here sends `arutd` a
/// signal or calls into it at all, only closes the pipe end `ChildHost` would
/// otherwise have held open. That is exactly what the OS also does to every
/// pipe a process held when the process is killed outright, so this proves
/// the daemon does not need its parent's `Drop` to run to notice it is gone.
#[tokio::test]
async fn a_daemon_exits_once_its_parents_stdin_pipe_closes() {
    let dir = scratch_dir("stdin-eof");
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_arutd"))
        .env("ARUT_DATA", dir.join("data.pb"))
        .env("ARUT_ADDRESS", "127.0.0.1:0")
        .env_remove("ARUT_SOCKET")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut line = String::new();
    tokio::io::AsyncBufReadExt::read_line(&mut tokio::io::BufReader::new(stdout), &mut line)
        .await
        .unwrap();
    assert_eq!(line.trim(), "READY");

    drop(stdin);

    let status = tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .expect("the daemon did not exit once its stdin pipe closed")
        .unwrap();
    assert!(status.success(), "{status:?}");
    std::fs::remove_dir_all(dir).unwrap();
}

/// Two hosts over the same socket and node lease: the second must find the
/// first still there and reuse it, because actually spawning a second `arutd`
/// against an already-held lease can only fail (ADR 0011). A second `connect`
/// that still returns a working channel is therefore proof of reuse, not just
/// an absence of errors.
#[tokio::test]
async fn a_second_child_host_reuses_a_running_daemon() {
    let dir = scratch_dir("reuse");
    let socket = dir.join("node.sock");
    let data = dir.join("data.pb");
    let shared_spawner = spawner();

    let first = ChildHost {
        executable: env!("CARGO_BIN_EXE_arutd").into(),
        socket: socket.clone(),
        data: data.clone(),
        spawner: shared_spawner.clone(),
    };
    let first_channel = first.connect().await.unwrap();
    assert_channel_works(first_channel.clone(), "from the first host").await;

    let second = ChildHost {
        executable: env!("CARGO_BIN_EXE_arutd").into(),
        socket,
        data,
        spawner: shared_spawner,
    };
    let second_channel = second
        .connect()
        .await
        .expect("a live daemon at the socket should be reused, not raced for the lease");
    assert_channel_works(second_channel, "from the second host").await;

    drop(first_channel);
    std::fs::remove_dir_all(dir).unwrap();
}

/// A socket file with nothing behind it -- left by a crash, or by a version
/// that did not yet clean up after itself -- must not make `connect` fail: it
/// is deleted and a fresh daemon takes its place.
#[tokio::test]
async fn a_stale_socket_is_cleaned_and_a_fresh_daemon_spawned() {
    use std::os::unix::fs::FileTypeExt;

    let dir = scratch_dir("stale-socket");
    let socket = dir.join("node.sock");
    std::fs::write(&socket, b"not a socket").unwrap();

    let host = ChildHost {
        executable: env!("CARGO_BIN_EXE_arutd").into(),
        socket: socket.clone(),
        data: dir.join("data.pb"),
        spawner: spawner(),
    };
    let channel = host
        .connect()
        .await
        .expect("a stale socket file should be cleaned up rather than failing connect");
    assert_channel_works(channel.clone(), "past the stale socket").await;

    assert!(std::fs::metadata(&socket).unwrap().file_type().is_socket());
    drop(channel);
    std::fs::remove_dir_all(dir).unwrap();
}

/// The lease arm of the readiness handshake (ADR 0011): a daemon spawned
/// against a data directory whose lease is already held exits saying so, and
/// `connect` reports that reason rather than a bare `Unavailable`.
#[tokio::test]
async fn a_daemon_spawned_against_a_held_lease_reports_the_lease() {
    let dir = scratch_dir("lease-held");
    let data = dir.join("data.pb");
    let held = arut_runtime_local::LocalRuntime::open(data.clone(), "chat").unwrap();

    let host = ChildHost {
        executable: env!("CARGO_BIN_EXE_arutd").into(),
        socket: dir.join("node.sock"),
        data,
        spawner: spawner(),
    };
    let error = host.connect().await.err().expect("the lease is held");

    assert_eq!(Readiness::from_status(&error), Some(Readiness::LeaseHeld));
    drop(held);
    std::fs::remove_dir_all(dir).unwrap();
}
