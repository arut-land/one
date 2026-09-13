use arut_feature_chat::{ChatServiceImpl, composer::ComposerServiceImpl};
use arut_protocol::chat::{
    composer::v1::{
        ComposerService, GetComposerRequest, GetComposerResponse, ReplaceComposerRequest,
        ReplaceComposerResponse, WatchComposerRequest, WatchComposerResponse,
    },
    v1::{
        ChatService, ListConversationsRequest, ListConversationsResponse, SendMessageRequest,
        SendMessageResponse, StartChatRequest, StartChatResponse,
    },
};
use arut_rpc::{Code, Request, Response, RpcFuture, RpcStream, Status};
use std::{fs::File, sync::Arc};

/// These local services perform storage I/O while constructing their RPC future.
/// Keep that dispatch on the blocking pool; poll the returned future on Tokio.
pub(crate) struct Blocking<S> {
    service: Arc<S>,
    lease: Arc<File>,
}

impl<S: Send + Sync + 'static> Blocking<S> {
    pub(crate) fn new(service: S, lease: Arc<File>) -> Self {
        Self {
            service: Arc::new(service),
            lease,
        }
    }

    fn run<T: Send + 'static>(
        &self,
        call: impl FnOnce(Arc<S>) -> RpcFuture<T> + Send + 'static,
    ) -> RpcFuture<T> {
        let service = self.service.clone();
        let lease = self.lease.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let _lease = lease;
                call(service)
            })
            .await
            .map_err(|_| Status::new(Code::Internal, "local service stopped"))?
            .await
        })
    }
}

impl ComposerService for Blocking<ComposerServiceImpl> {
    fn get_composer(
        &self,
        request: Request<GetComposerRequest>,
    ) -> RpcFuture<Response<GetComposerResponse>> {
        self.run(|service| service.get_composer(request))
    }
    fn replace_composer(
        &self,
        request: Request<ReplaceComposerRequest>,
    ) -> RpcFuture<Response<ReplaceComposerResponse>> {
        self.run(|service| service.replace_composer(request))
    }
    fn watch_composer(
        &self,
        request: Request<WatchComposerRequest>,
    ) -> RpcFuture<Response<RpcStream<WatchComposerResponse>>> {
        self.run(|service| service.watch_composer(request))
    }
}

impl ChatService for Blocking<ChatServiceImpl> {
    fn list_conversations(
        &self,
        request: Request<ListConversationsRequest>,
    ) -> RpcFuture<Response<ListConversationsResponse>> {
        self.run(|service| service.list_conversations(request))
    }
    fn send_message(
        &self,
        request: Request<SendMessageRequest>,
    ) -> RpcFuture<Response<SendMessageResponse>> {
        self.run(|service| service.send_message(request))
    }
    fn start_chat(
        &self,
        request: Request<StartChatRequest>,
    ) -> RpcFuture<Response<StartChatResponse>> {
        self.run(|service| service.start_chat(request))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn storage_dispatch_does_not_run_on_the_async_worker() {
        let path = std::env::temp_dir().join(format!("arut-blocking-test-{}", std::process::id()));
        let service = Blocking::new((), Arc::new(File::create(&path).unwrap()));
        let worker = std::thread::current().id();
        let dispatched = service
            .run(move |_| {
                assert_ne!(std::thread::current().id(), worker);
                Box::pin(async { Ok(42) })
            })
            .await
            .unwrap();
        assert_eq!(dispatched, 42);
        drop(service);
        std::fs::remove_file(path).unwrap();
    }
}
