use arut_product_session::hosting::{Host, HostMode};
use arut_rpc::{Code, Request, Response, RpcChannel, RpcFuture, RpcStream, Spawner, Status};
use std::{future::Future, pin::Pin, sync::Arc};

pub struct TokioSpawner(pub tokio::runtime::Handle);
impl Spawner for TokioSpawner {
    fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) {
        self.0.spawn(future);
    }
}

/// Ensures foreign callers may poll RPC results on their own schedulers.
pub struct ScheduledChannel {
    channel: Arc<dyn RpcChannel>,
    spawner: Arc<dyn Spawner>,
}
impl ScheduledChannel {
    pub fn new(channel: Arc<dyn RpcChannel>, spawner: Arc<dyn Spawner>) -> Self {
        Self { channel, spawner }
    }
    fn run<T: Send + 'static>(&self, future: RpcFuture<T>) -> RpcFuture<T> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.spawner.spawn(Box::pin(async move {
            let _ = sender.send(future.await);
        }));
        Box::pin(async move {
            receiver
                .await
                .map_err(|_| Status::new(Code::Cancelled, "host stopped"))?
        })
    }
}
impl RpcChannel for ScheduledChannel {
    fn unary(&self, procedure: &str, request: Request<Vec<u8>>) -> RpcFuture<Response<Vec<u8>>> {
        self.run(self.channel.unary(procedure, request))
    }
    fn server_stream(
        &self,
        procedure: &str,
        request: Request<Vec<u8>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
        self.run(self.channel.server_stream(procedure, request))
    }
    fn client_stream(
        &self,
        procedure: &str,
        request: Request<RpcStream<Vec<u8>>>,
    ) -> RpcFuture<Response<Vec<u8>>> {
        self.run(self.channel.client_stream(procedure, request))
    }
    fn bidirectional(
        &self,
        procedure: &str,
        request: Request<RpcStream<Vec<u8>>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
        self.run(self.channel.bidirectional(procedure, request))
    }
}

pub struct ChannelHost {
    mode: HostMode,
    channel: Arc<dyn RpcChannel>,
}
impl ChannelHost {
    pub fn new(mode: HostMode, channel: Arc<dyn RpcChannel>, spawner: Arc<dyn Spawner>) -> Self {
        Self {
            mode,
            channel: Arc::new(ScheduledChannel::new(channel, spawner)),
        }
    }
}
impl Host for ChannelHost {
    fn mode(&self) -> HostMode {
        self.mode
    }
    fn connect(&self) -> RpcFuture<Arc<dyn RpcChannel>> {
        let channel = Arc::clone(&self.channel);
        Box::pin(async move { Ok(channel) })
    }
}

/// Executor ownership is retained by the native composition root.
pub fn desktop_executor() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn root_supplies_executor_and_host_channel() {
        let spawner: Arc<dyn Spawner> = Arc::new(TokioSpawner(tokio::runtime::Handle::current()));
        let (tx, rx) = tokio::sync::oneshot::channel();
        spawner.spawn(Box::pin(async move {
            tx.send(tokio::runtime::Handle::try_current().is_ok())
                .unwrap();
        }));
        assert!(rx.await.unwrap());
        let host = ChannelHost::new(
            HostMode::InProcess,
            Arc::new(arut_rpc::RpcRegistry::default()),
            spawner,
        );
        assert_eq!(host.mode(), HostMode::InProcess);
        let error = host
            .connect()
            .await
            .unwrap()
            .unary("/missing", Request::new(vec![]))
            .await
            .unwrap_err();
        assert_eq!(error.code, Code::Unimplemented);
    }
}
