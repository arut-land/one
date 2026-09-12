//! Behavioral suites shared by every implementation of each port.
//!
//! RPC checks cover unary and server streams over the registry, TCP Connect, and
//! Unix IPC. Fact-log and key-value checks cover memory, directory, and redb;
//! blob checks cover memory and directory. They exercise retries, fencing,
//! atomic decisions, compaction, content addressing, and independent writers.

use arut_rpc::{
    Code, Metadata, MethodDescriptor, Request, Response, RpcChannel, RpcFuture, RpcService,
    RpcStream, ServiceDescriptor, Status, StreamingKind,
};
use arut_storage::{BlobStore, FactLog, KeyValue, Snapshot, StorageError, digest};
use futures_util::StreamExt;

pub async fn rpc(channel: &dyn RpcChannel) {
    let mut request = Request::new(vec![0, 128, 255]);
    request.metadata.insert("x-test-bin", [0, 128, 255]);
    request.metadata.insert("x-test", b"scope".to_vec());
    let response = channel.unary(ECHO, request).await.unwrap();
    assert_eq!(response.message, [0, 128, 255]);
    assert_eq!(
        response.metadata.get("x-test-bin"),
        Some([0, 128, 255].as_slice())
    );
    assert_eq!(response.metadata.get("x-test"), Some(b"scope".as_slice()));
    let error = channel
        .unary(ERROR, Request::new(vec![]))
        .await
        .unwrap_err();
    assert_eq!(error.code, Code::FailedPrecondition);
    assert_eq!(error.details, [7, 8, 9]);
    let mut stream = channel
        .server_stream(STREAM, Request::new(vec![42]))
        .await
        .unwrap()
        .message;
    assert_eq!(stream.next().await.unwrap().unwrap(), [42]);
    assert_eq!(stream.next().await.unwrap().unwrap(), [43]);
    assert!(stream.next().await.is_none());
    let mut stream = channel
        .server_stream(STREAM_ERROR, Request::new(vec![]))
        .await
        .unwrap()
        .message;
    assert_eq!(stream.next().await.unwrap().unwrap(), [1]);
    let error = stream.next().await.unwrap().unwrap_err();
    assert_eq!(error.code, Code::FailedPrecondition);
    assert_eq!(error.details, [7, 8, 9]);
    assert!(stream.next().await.is_none());
    assert_eq!(
        channel
            .unary("/arut.conformance.v1.Echo/Unknown", Request::new(vec![]))
            .await
            .unwrap_err()
            .code,
        Code::Unimplemented
    );
}

pub fn fact_log(log: &dyn FactLog<String>) {
    assert!(log.read_from(0).unwrap().is_empty());
    let mut decisions = 0;
    assert!(
        log.commit(Some(0), 1, "unused", &mut |_, records, duplicate| {
            decisions += 1;
            assert!(records.is_empty());
            assert!(duplicate.is_none());
            Ok(None)
        })
        .unwrap()
        .is_none()
    );
    assert_eq!(decisions, 1);
    let first = log
        .commit(Some(0), 1, "one", &mut |_, _, _| Ok(Some("first".into())))
        .unwrap()
        .unwrap();
    assert!(
        log.commit(Some(0), 1, "one", &mut |_, records, duplicate| {
            assert_eq!(records, std::slice::from_ref(&first));
            assert_eq!(duplicate, Some(first.clone()));
            Ok(None)
        })
        .unwrap()
        .is_none()
    );
    assert_eq!(first.sequence, 1);
    assert_eq!(
        log.append(0, 1, "one", "different retry".into()).unwrap(),
        first
    );
    assert_eq!(
        log.append(0, 1, "two", "second".into()).unwrap_err(),
        StorageError::Conflict { actual: 1 }
    );
    log.append(1, 2, "two", "second".into()).unwrap();
    assert_eq!(
        log.append(2, 1, "one", "stale retry".into()).unwrap_err(),
        StorageError::Epoch { current: 2 }
    );
    assert_eq!(log.read_from(1).unwrap().len(), 1);
    assert_eq!(
        log.commit(Some(2), 1, "one", &mut |_, _, _| panic!(
            "stale epoch must not decide"
        )),
        Err(StorageError::Epoch { current: 2 })
    );
    assert_eq!(
        log.commit(Some(3), 2, "future", &mut |_, _, _| panic!(
            "future cursor must not decide"
        )),
        Err(StorageError::Conflict { actual: 2 })
    );
    assert!(matches!(
        log.compact(2),
        Err(StorageError::SnapshotRequired)
    ));
    let snapshot = Snapshot {
        sequence: 2,
        epoch: 2,
        data: b"projection".to_vec(),
    };
    log.save_snapshot(snapshot.clone()).unwrap();
    assert_eq!(log.snapshot().unwrap(), Some(snapshot));
    log.compact(2).unwrap();
    assert_eq!(
        log.read_from(0).unwrap_err(),
        StorageError::CursorUnavailable { through: 2 }
    );
    assert!(
        log.commit(Some(2), 2, "one", &mut |_, records, duplicate| {
            assert!(records.is_empty());
            assert_eq!(duplicate, Some(first.clone()));
            Ok(None)
        })
        .unwrap()
        .is_none()
    );
    assert_eq!(
        log.commit(Some(0), 2, "one", &mut |_, _, _| panic!(
            "compacted cursor must not decide"
        )),
        Err(StorageError::CursorUnavailable { through: 2 })
    );
    assert_eq!(log.outcome_of("one").unwrap(), Some(first));
    assert_eq!(
        log.append(2, 2, "three", "third".into()).unwrap().sequence,
        3
    );
    assert_eq!(log.read_from(2).unwrap().len(), 1);
}
/// The address every implementation must mint for the same bytes.
///
/// BLAKE3, lowercase hex, 64 characters -- pinned as a literal rather than
/// recomputed, because the point is that the format cannot drift: the same
/// bytes must land on the same address in this process, in a directory, in
/// SQLite, and in whatever `iroh-blobs` fetches them by later.
const CONTENT_DIGEST: &str = "3fba5250be9ac259c56e7250c526bc83bacb4be825f2799d3d59e5b4878dd74e";
const EMPTY_DIGEST: &str = "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262";

/// Blobs are addressed by the BLAKE3 of their content, everywhere alike.
pub fn blobs(store: &dyn BlobStore) {
    let id = store.put_blob(b"content").unwrap();
    assert_eq!(id, CONTENT_DIGEST);
    assert_eq!(id, digest(b"content"));
    assert_eq!(id.len(), 64);
    assert!(
        id.bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    );
    assert_eq!(id, store.put_blob(b"content").unwrap());
    assert_eq!(store.get_blob(&id).unwrap(), Some(b"content".to_vec()));
    assert_ne!(id, store.put_blob(b"different").unwrap());

    let empty = store.put_blob(b"").unwrap();
    assert_eq!(empty, EMPTY_DIGEST);
    assert_eq!(store.get_blob(&empty).unwrap(), Some(Vec::new()));

    let large = vec![0xa5; 1 << 20];
    let large_id = store.put_blob(&large).unwrap();
    assert_eq!(store.get_blob(&large_id).unwrap(), Some(large));

    // A well-formed digest nobody stored is absent, not an error.
    assert!(store.get_blob(&"0".repeat(64)).unwrap().is_none());
    // Anything that is not a digest is refused before it can reach a path.
    for malformed in [
        "",
        "0",
        &"0".repeat(63),
        &"g".repeat(64),
        &CONTENT_DIGEST.to_uppercase(),
        "../values/key",
    ] {
        assert_eq!(store.get_blob(malformed), Err(StorageError::Corrupt));
    }
}
pub fn key_value(store: &dyn KeyValue) {
    assert!(store.get("../scope/key").unwrap().is_none());
    store.put("../scope/key", b"draft").unwrap();
    store.put("second", b"independent").unwrap();
    assert_eq!(store.get("../scope/key").unwrap(), Some(b"draft".to_vec()));
    store.remove("../scope/key").unwrap();
    assert!(store.get("../scope/key").unwrap().is_none());
    assert_eq!(store.get("second").unwrap(), Some(b"independent".to_vec()));
}

/// The service every channel implementation is measured against.
pub struct Echo;
/// Connect requires `/package.Service/Method`, so the suite's own procedures
/// have that shape too: a framing implementation may reject anything else.
pub const ECHO: &str = "/arut.conformance.v1.Echo/Echo";
pub const ERROR: &str = "/arut.conformance.v1.Echo/Error";
pub const STREAM: &str = "/arut.conformance.v1.Echo/Stream";
pub const STREAM_ERROR: &str = "/arut.conformance.v1.Echo/StreamError";
static ECHO_METHODS: &[MethodDescriptor] = &[
    method("Echo", ECHO, StreamingKind::Unary),
    method("Error", ERROR, StreamingKind::Unary),
    method("Stream", STREAM, StreamingKind::Server),
    method("StreamError", STREAM_ERROR, StreamingKind::Server),
];
pub static ECHO_DESCRIPTOR: ServiceDescriptor = ServiceDescriptor {
    name: "Echo",
    package: "arut.conformance.v1",
    version: "v1",
    methods: ECHO_METHODS,
};
const fn method(
    name: &'static str,
    procedure: &'static str,
    streaming: StreamingKind,
) -> MethodDescriptor {
    MethodDescriptor {
        name,
        procedure,
        input: "arut.conformance.v1.Bytes",
        output: "arut.conformance.v1.Bytes",
        streaming,
    }
}
impl RpcService for Echo {
    fn descriptor(&self) -> &'static ServiceDescriptor {
        &ECHO_DESCRIPTOR
    }
}
fn error() -> Status {
    let mut error = Status::new(Code::FailedPrecondition, "typed failure");
    error.details = vec![7, 8, 9];
    error
}
impl RpcChannel for Echo {
    fn unary(&self, procedure: &str, request: Request<Vec<u8>>) -> RpcFuture<Response<Vec<u8>>> {
        let result = match procedure {
            ECHO => {
                let mut metadata = Metadata::default();
                for key in ["x-test", "x-test-bin"] {
                    if let Some(value) = request.metadata.get(key) {
                        metadata.insert(key, value.to_vec());
                    }
                }
                Ok(Response {
                    message: request.message,
                    metadata,
                })
            }
            ERROR => Err(error()),
            _ => Err(Status::unimplemented(procedure)),
        };
        Box::pin(async { result })
    }
    fn server_stream(
        &self,
        procedure: &str,
        request: Request<Vec<u8>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
        let items = match procedure {
            STREAM => vec![Ok(request.message), Ok(vec![43])],
            STREAM_ERROR => vec![Ok(vec![1]), Err(error())],
            _ => vec![Err(Status::unimplemented(procedure))],
        };
        Box::pin(async {
            Ok(Response::new(
                Box::pin(futures_util::stream::iter(items)) as RpcStream<Vec<u8>>
            ))
        })
    }
    fn client_stream(
        &self,
        p: &str,
        _: Request<RpcStream<Vec<u8>>>,
    ) -> RpcFuture<Response<Vec<u8>>> {
        let error = Status::unimplemented(p);
        Box::pin(async { Err(error) })
    }
    fn bidirectional(
        &self,
        p: &str,
        _: Request<RpcStream<Vec<u8>>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
        let error = Status::unimplemented(p);
        Box::pin(async { Err(error) })
    }
}
