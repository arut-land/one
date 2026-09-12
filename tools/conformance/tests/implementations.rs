use arut_conformance::*;
use arut_storage::{Directory, FactLog, MemoryLog, MemoryStore};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
fn directory() -> std::path::PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "arut-conformance-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}
#[test]
fn memory_storage() {
    fact_log(&MemoryLog::default());
    let store = MemoryStore::default();
    blobs(&store);
    key_value(&store);
}
#[test]
fn directory_storage_reopens_after_compaction() {
    let path = directory();
    let store = Directory::open(&path).unwrap();
    fact_log(&store.log("suite").unwrap());
    blobs(&store);
    key_value(&store);
    drop(store);
    let reopened = Directory::open(&path)
        .unwrap()
        .log::<String>("suite")
        .unwrap();
    assert_eq!(reopened.read_from(2).unwrap()[0].fact, "third");
    assert_eq!(reopened.outcome_of("one").unwrap().unwrap().fact, "first");
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn independent_writers_compare_and_append_atomically() {
    let path = directory();
    let one = Directory::open(&path)
        .unwrap()
        .log::<String>("suite")
        .unwrap();
    let two = Directory::open(&path)
        .unwrap()
        .log::<String>("suite")
        .unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let other = barrier.clone();
    let thread = std::thread::spawn(move || {
        other.wait();
        one.append(0, 1, "one", "one".into())
    });
    barrier.wait();
    let result = two.append(0, 1, "two", "two".into());
    assert_ne!(thread.join().unwrap().is_ok(), result.is_ok());
    assert_eq!(two.read_from(0).unwrap().len(), 1);
    std::fs::remove_dir_all(path).unwrap();
}
#[tokio::test]
async fn in_process_registry_rpc() {
    let registry = arut_rpc::RpcRegistry::default()
        .register(Arc::new(Echo))
        .unwrap();
    rpc(&registry).await;
}
#[tokio::test]
async fn connect_http_rpc() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            arut_transport_connect_http::router(Arc::new(Echo)),
        )
        .await
        .unwrap();
    });
    rpc(&arut_transport_connect_http::HttpRpcChannel::new(format!(
        "http://{address}"
    )))
    .await;
    server.abort();
}
#[cfg(unix)]
#[tokio::test]
async fn unix_socket_rpc() {
    let path = directory();
    std::fs::create_dir_all(&path).unwrap();
    let socket = path.join("node.sock");
    let listener = tokio::net::UnixListener::bind(&socket).unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            arut_transport_connect_http::router(Arc::new(Echo)),
        )
        .await
        .unwrap();
    });
    rpc(&arut_transport_ipc::unix_socket(&socket).unwrap()).await;
    server.abort();
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn framing_handles_fragmentation_limits_and_invalid_flags() {
    use arut_transport_connect_http::framing::*;
    let envelope = envelope(0, b"message");
    let mut buffer = vec![];
    for byte in &envelope[..envelope.len() - 1] {
        buffer.push(*byte);
        assert!(take(&mut buffer).unwrap().is_none());
    }
    buffer.push(*envelope.last().unwrap());
    assert_eq!(take(&mut buffer).unwrap(), Some((0, b"message".to_vec())));
    assert!(buffer.is_empty());
    assert!(take(&mut vec![1, 0, 0, 0, 0]).is_err());
    let mut too_large = vec![0];
    too_large.extend_from_slice(&((MAX_MESSAGE + 1) as u32).to_be_bytes());
    assert!(take(&mut too_large).is_err());
}
