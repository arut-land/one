//! # Conformance
//!
//! Reusable suites exercise unary and server-stream RPC ordering, metadata, typed
//! errors and stream termination; fact-log sequencing, fencing, retries, snapshots,
//! and compaction; blob content addressing; and independent key-value updates.
//!
//! The same RPC suite runs against memory, TCP Connect, and Unix IPC. Storage suites
//! run against memory and directory implementations. Extra tests cover independent
//! writers, reopening compacted logs, and fragmented/malformed Connect envelopes.

//! Shared behavioral suites. An implementation must pass without changing them.
use arut_rpc::{
    Code, Metadata, MethodDescriptor, Request, Response, RpcChannel, RpcFuture, RpcService,
    RpcStream, ServiceDescriptor, Status, StreamingKind,
};
use arut_storage::{BlobStore, FactLog, KeyValue, Snapshot, StorageError};
use futures_util::StreamExt;

pub async fn rpc(channel: &dyn RpcChannel) {
    let mut request = Request::new(vec![0, 128, 255]);
    request.metadata.insert("x-test-bin", [0, 128, 255]);
    request.metadata.insert("x-test", b"scope".to_vec());
    let response = channel.unary("/echo", request).await.unwrap();
    assert_eq!(response.message, [0, 128, 255]);
    assert_eq!(
        response.metadata.get("x-test-bin"),
        Some([0, 128, 255].as_slice())
    );
    assert_eq!(response.metadata.get("x-test"), Some(b"scope".as_slice()));
    let error = channel
        .unary("/error", Request::new(vec![]))
        .await
        .unwrap_err();
    assert_eq!(error.code, Code::FailedPrecondition);
    assert_eq!(error.details, [7, 8, 9]);
    let mut stream = channel
        .server_stream("/stream", Request::new(vec![42]))
        .await
        .unwrap()
        .message;
    assert_eq!(stream.next().await.unwrap().unwrap(), [42]);
    assert_eq!(stream.next().await.unwrap().unwrap(), [43]);
    assert!(stream.next().await.is_none());
    let mut stream = channel
        .server_stream("/stream-error", Request::new(vec![]))
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
            .unary("/unknown", Request::new(vec![]))
            .await
            .unwrap_err()
            .code,
        Code::Unimplemented
    );
}

pub fn fact_log(log: &dyn FactLog<String>) {
    assert!(log.read_from(0).unwrap().is_empty());
    let first = log.append(0, 1, "one", "first".into()).unwrap();
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
    assert_eq!(log.outcome_of("one").unwrap(), Some(first));
    assert_eq!(
        log.append(2, 2, "three", "third".into()).unwrap().sequence,
        3
    );
    assert_eq!(log.read_from(2).unwrap().len(), 1);
}
pub fn blobs(store: &dyn BlobStore) {
    let id = store.put_blob(b"content").unwrap();
    assert_eq!(id, store.put_blob(b"content").unwrap());
    assert_eq!(store.get_blob(&id).unwrap(), Some(b"content".to_vec()));
    assert_ne!(id, store.put_blob(b"different").unwrap());
    assert!(store.get_blob(&"0".repeat(64)).unwrap().is_none());
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
static ECHO_METHODS: &[MethodDescriptor] = &[
    method("Echo", "/echo", StreamingKind::Unary),
    method("Error", "/error", StreamingKind::Unary),
    method("Stream", "/stream", StreamingKind::Server),
    method("StreamError", "/stream-error", StreamingKind::Server),
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
            "/echo" => {
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
            "/error" => Err(error()),
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
            "/stream" => vec![Ok(request.message), Ok(vec![43])],
            "/stream-error" => vec![Ok(vec![1]), Err(error())],
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
