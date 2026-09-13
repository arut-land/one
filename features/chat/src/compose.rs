//! One construction path for direct clients and remote routers.
use crate::{
    composer::{ComposerAuthority, ComposerServiceImpl},
    ports::ChatRuntime,
    service::ChatServiceImpl,
};
use arut_protocol::chat::{
    composer::v1::{ComposerServiceClient, ComposerServiceRouter},
    v1::{ChatServiceClient, ChatServiceRouter},
};
use arut_rpc::{RpcService, ServiceMetadata, ServiceRegistration};
use std::sync::Arc;

pub type ComposeError = arut_storage::StorageError;

/// Generated clients sharing the services owned by the composed feature.
#[derive(Clone)]
pub struct ChatClients {
    pub chat: ChatServiceClient,
    pub composer: ComposerServiceClient,
}

/// Owns the service graph; neither a host nor a session constructs its internals.
pub struct ChatFeature {
    clients: ChatClients,
    routers: Vec<Arc<dyn RpcService>>,
}
impl ChatFeature {
    pub fn routers(&self) -> impl Iterator<Item = Arc<dyn RpcService>> {
        self.routers.clone().into_iter()
    }
    pub fn clients(&self) -> ChatClients {
        self.clients.clone()
    }
    /// The manifest uses the very descriptors served by this feature.
    pub fn registrations(&self) -> impl Iterator<Item = ServiceRegistration> {
        self.routers()
            .map(|router| ServiceRegistration::new(router.descriptor(), ServiceMetadata::default()))
    }
}

pub fn compose<R: ChatRuntime>(runtime: Arc<R>) -> Result<ChatFeature, ComposeError> {
    let authority = Arc::new(ComposerAuthority::with_store(runtime.drafts()));
    let chat = Arc::new(ChatServiceImpl::new(
        authority.clone(),
        runtime.log("chat")?,
        runtime.clone(),
    )?);
    let composer = Arc::new(ComposerServiceImpl::new(authority).with_runtime(runtime));
    Ok(ChatFeature {
        clients: ChatClients {
            chat: ChatServiceClient::direct(chat.clone()),
            composer: ComposerServiceClient::direct(composer.clone()),
        },
        routers: vec![
            Arc::new(ChatServiceRouter::new(chat)),
            Arc::new(ComposerServiceRouter::new(composer)),
        ],
    })
}

impl ChatClients {
    /// Bind this feature's generated clients to the route chosen by the root.
    pub fn remote(channel: Arc<dyn arut_rpc::RpcChannel>) -> Self {
        Self {
            chat: ChatServiceClient::remote(channel.clone()),
            composer: ComposerServiceClient::remote(channel),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ports::{Clock, NativeIds},
        test_support::MemoryPorts,
    };
    use arut_protocol::chat::v1::StartChatRequest;
    use arut_rpc::{Request, RpcRegistry};
    use futures_executor::block_on;

    #[test]
    fn direct_and_remote_clients_share_services_and_manifest_descriptors() {
        let mut ports = MemoryPorts::new(Arc::new(NativeIds));
        ports.now = 123;
        assert_eq!(ports.now(), 123);
        let feature = compose(Arc::new(ports)).unwrap();
        let mut registry = RpcRegistry::default();
        for router in feature.routers() {
            registry = registry.register(router).unwrap();
        }
        let advertised: Vec<_> = feature.registrations().map(|r| r.descriptor.name).collect();
        assert_eq!(
            advertised,
            registry
                .registrations()
                .map(|r| r.descriptor.name)
                .collect::<Vec<_>>()
        );
        let remote = ChatServiceClient::remote(Arc::new(registry));
        let request = StartChatRequest {
            pending_scope_id: "one".into(),
            command_id: "01900000-0000-7000-8000-000000000001".into(),
            expected_revision: 0,
            text: String::new(),
        };
        let direct = feature.clients();
        drop(feature);
        let first = block_on(direct.chat.start_chat(Request::new(request.clone())))
            .unwrap()
            .message;
        let retry = block_on(remote.start_chat(Request::new(request)))
            .unwrap()
            .message;
        assert_eq!(first, retry);
        assert!(
            first
                .messages
                .iter()
                .all(|message| message.accepted_at_ms == 123)
        );
    }
}
