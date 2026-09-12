use arut_conformance::*;
use arut_storage::{BlobStore, Directory, FactLog, KeyValue, MemoryLog, MemoryStore, Redb};
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
    let blob = store.put_blob(b"kept across restarts").unwrap();
    drop(store);
    let reopened = Directory::open(&path).unwrap();
    assert_eq!(
        reopened.get_blob(&blob).unwrap(),
        Some(b"kept across restarts".to_vec())
    );
    let log = reopened.log::<String>("suite").unwrap();
    assert_eq!(log.read_from(2).unwrap()[0].fact, "third");
    assert_eq!(log.outcome_of("one").unwrap().unwrap().fact, "first");
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn redb_storage_reopens_after_compaction() {
    let path = directory();
    std::fs::create_dir_all(&path).unwrap();
    let file = path.join("node.redb");
    let store = Redb::open(&file).unwrap();
    fact_log(&store.log::<String>());
    key_value(&store);
    let kept = store.log::<String>();
    assert_eq!(kept.read_from(2).unwrap()[0].fact, "third");
    drop(kept);
    drop(store);

    let reopened = Redb::open(&file).unwrap();
    let log = reopened.log::<String>();
    assert_eq!(log.read_from(2).unwrap()[0].fact, "third");
    assert_eq!(log.outcome_of("one").unwrap().unwrap().fact, "first");
    reopened.put("kept", b"across restarts").unwrap();
    drop(log);
    drop(reopened);

    let again = Redb::open(&file).unwrap();
    assert_eq!(
        again.get("kept").unwrap(),
        Some(b"across restarts".to_vec())
    );
    drop(again);
    std::fs::remove_dir_all(path).unwrap();
}

/// One node owns its file. redb locks it, so a second opener is refused rather
/// than allowed to interleave; other processes reach these facts over RPC.
#[test]
fn redb_refuses_a_second_holder_of_the_same_file() {
    let path = directory();
    std::fs::create_dir_all(&path).unwrap();
    let file = path.join("node.redb");
    let held = Redb::open(&file).unwrap();

    assert!(Redb::open(&file).is_err());

    drop(held);
    assert!(Redb::open(&file).is_ok());
    std::fs::remove_dir_all(path).unwrap();
}

/// Inside the owning process, two appenders behave as the directory log's two
/// lock holders do: redb serializes the write transactions and one loses.
#[test]
fn redb_writers_compare_and_append_atomically() {
    let path = directory();
    std::fs::create_dir_all(&path).unwrap();
    let store = Redb::open(path.join("node.redb")).unwrap();
    let one = store.log::<String>();
    let two = store.log::<String>();
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
    drop(two);
    drop(store);
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
    let envelope = envelope(0, b"message").unwrap();
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

#[tokio::test]
async fn http_bounds_chunked_unary_and_error_bodies_and_outbound_requests() {
    use arut_rpc::{Code, Request, RpcChannel};
    use arut_transport_connect_http::{
        HttpRpcChannel,
        framing::{MAX_MESSAGE, envelope},
    };
    let oversized = vec![0; MAX_MESSAGE + 1];
    assert_eq!(
        envelope(0, &oversized).unwrap_err().code,
        Code::ResourceExhausted
    );
    let unreachable = HttpRpcChannel::new("http://127.0.0.1:1");
    assert_eq!(
        unreachable
            .unary("/large", Request::new(oversized.clone()))
            .await
            .unwrap_err()
            .code,
        Code::ResourceExhausted
    );
    let error = unreachable
        .server_stream("/large", Request::new(oversized))
        .await
        .err()
        .unwrap();
    assert_eq!(error.code, Code::ResourceExhausted);

    for status in [
        axum::http::StatusCode::OK,
        axum::http::StatusCode::BAD_REQUEST,
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let router = axum::Router::new().route(
            "/large",
            axum::routing::post(move || async move {
                let chunks = futures_util::stream::iter(
                    (0..=MAX_MESSAGE / 4096)
                        .map(|_| Ok::<_, std::convert::Infallible>(vec![0; 4096])),
                );
                (status, axum::body::Body::from_stream(chunks))
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let channel = HttpRpcChannel::new(format!("http://{address}"));
        assert_eq!(
            channel
                .unary("/large", Request::new(vec![]))
                .await
                .unwrap_err()
                .code,
            Code::ResourceExhausted
        );
        if !status.is_success() {
            let error = channel
                .server_stream("/large", Request::new(vec![]))
                .await
                .err()
                .unwrap();
            assert_eq!(error.code, Code::ResourceExhausted);
        }
        server.abort();
    }
}
