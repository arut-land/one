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
    fn run<T: Send + 'static>(
        &self,
        call: impl FnOnce(Arc<dyn RpcChannel>) -> RpcFuture<T> + Send + 'static,
    ) -> RpcFuture<T> {
        let channel = self.channel.clone();
        let (mut sender, receiver) = tokio::sync::oneshot::channel();
        self.spawner.spawn(Box::pin(async move {
            tokio::select! {
                biased;
                () = sender.closed() => {},
                result = async move { call(channel).await } => { let _ = sender.send(result); }
            }
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
        let procedure = procedure.to_owned();
        self.run(move |channel| channel.unary(&procedure, request))
    }
    fn server_stream(
        &self,
        procedure: &str,
        request: Request<Vec<u8>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
        let procedure = procedure.to_owned();
        self.run(move |channel| channel.server_stream(&procedure, request))
    }
    fn client_stream(
        &self,
        procedure: &str,
        request: Request<RpcStream<Vec<u8>>>,
    ) -> RpcFuture<Response<Vec<u8>>> {
        let procedure = procedure.to_owned();
        self.run(move |channel| channel.client_stream(&procedure, request))
    }
    fn bidirectional(
        &self,
        procedure: &str,
        request: Request<RpcStream<Vec<u8>>>,
    ) -> RpcFuture<Response<RpcStream<Vec<u8>>>> {
        let procedure = procedure.to_owned();
        self.run(move |channel| channel.bidirectional(&procedure, request))
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

    #[test]
    fn foreign_dispatch_uses_the_host_and_dropping_the_call_cancels_its_task() {
        struct Dropped(Option<tokio::sync::oneshot::Sender<()>>);
        impl Drop for Dropped {
            fn drop(&mut self) {
                let _ = self.0.take().unwrap().send(());
            }
        }
        let runtime = desktop_executor().unwrap();
        let channel = ScheduledChannel::new(
            Arc::new(arut_rpc::RpcRegistry::default()),
            Arc::new(TokioSpawner(runtime.handle().clone())),
        );
        let (started, ready) = tokio::sync::oneshot::channel();
        let (dropped, cancelled) = tokio::sync::oneshot::channel();
        let response: RpcFuture<()> = channel.run(move |_| {
            assert!(tokio::runtime::Handle::try_current().is_ok());
            let guard = Dropped(Some(dropped));
            Box::pin(async move {
                let _guard = guard;
                started.send(()).unwrap();
                std::future::pending().await
            })
        });
        runtime.block_on(ready).unwrap();
        drop(response);
        runtime.block_on(cancelled).unwrap();
    }

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
