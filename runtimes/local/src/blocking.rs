//! Runs erased RPC dispatch and its response future on the blocking pool.
//! Generated routers defer service calls until polling; moving only construction
//! would still perform synchronous storage I/O on the async worker.
use arut_rpc::{
    Code, Request, Response, RpcChannel, RpcFuture, RpcService, RpcStream, ServiceDescriptor,
    Status, Wrap, Wrapped,
};
use std::sync::Arc;

/// Holds the runtime for as long as a call is in flight, so a node's storage
/// outlives the last dispatch against it.
struct Offload<R>(Arc<R>);
impl<R: Send + Sync + 'static> Wrap for Offload<R> {
    fn wrap<T: Send + 'static>(
        &self,
        call: Box<dyn FnOnce() -> RpcFuture<T> + Send>,
    ) -> RpcFuture<T> {
        let runtime = self.0.clone();
        Box::pin(async move {
            let executor = tokio::runtime::Handle::current();
            tokio::task::spawn_blocking(move || {
                let _runtime = runtime;
                executor.block_on(call())
            })
            .await
            .map_err(|_| Status::new(Code::Internal, "local service stopped"))?
        })
    }
}

pub(crate) struct Blocking<R> {
    descriptor: &'static ServiceDescriptor,
    channel: Wrapped<Offload<R>>,
}
impl<R: Send + Sync + 'static> Blocking<R> {
    pub(crate) fn new(service: Arc<dyn RpcService>, runtime: Arc<R>) -> Self {
        Self {
            descriptor: service.descriptor(),
            channel: Wrapped::new(service, Offload(runtime)),
        }
    }
}
impl<R: Send + Sync + 'static> RpcService for Blocking<R> {
    fn descriptor(&self) -> &'static ServiceDescriptor {
        self.descriptor
    }
}
impl<R: Send + Sync + 'static> RpcChannel for Blocking<R> {
    fn unary(&self, procedure: &str, request: Request<Vec<u8>>) -> RpcFuture<Response<Vec<u8>>> {
        self.channel.unary(procedure, request)
    }
    fn server_stream(
        &self,
        procedure: &str,
        request: Request<Vec<u8>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
        self.channel.server_stream(procedure, request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Probe(std::thread::ThreadId);
    impl RpcService for Probe {
        fn descriptor(&self) -> &'static ServiceDescriptor {
            &ServiceDescriptor {
                name: "Probe",
                package: "test.v1",
                version: arut_rpc::Version { major: 1, minor: 0 },
                methods: &[],
            }
        }
    }
    impl RpcChannel for Probe {
        fn unary(&self, _: &str, _: Request<Vec<u8>>) -> RpcFuture<Response<Vec<u8>>> {
            let worker = self.0;
            assert_ne!(std::thread::current().id(), worker);
            Box::pin(async move {
                assert_ne!(std::thread::current().id(), worker);
                Ok(Response::new(vec![42]))
            })
        }
        fn server_stream(
            &self,
            _: &str,
            _: Request<Vec<u8>>,
        ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
            unreachable!()
        }
    }
    #[tokio::test(flavor = "current_thread")]
    async fn construction_and_polling_leave_the_async_worker_and_retain_runtime() {
        let runtime = Arc::new(());
        let weak = Arc::downgrade(&runtime);
        let service = Blocking::new(Arc::new(Probe(std::thread::current().id())), runtime);
        let response = service.unary("test", Request::new(vec![]));
        drop(service);
        assert!(weak.upgrade().is_some());
        assert_eq!(response.await.unwrap().message, [42]);
        assert!(weak.upgrade().is_none());
    }
}
