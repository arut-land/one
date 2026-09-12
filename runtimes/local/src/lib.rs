//! # Local runtime
//!
//! `arutd` hosts chat and composer services over Connect HTTP or a private Unix
//! socket. Directory storage contains immutable chat facts, raw blobs, and local
//! draft recovery values. ARUT_DATA selects the directory; ARUT_SOCKET selects IPC;
//! ARUT_ADDRESS selects TCP.
//!
//! TokioSpawner receives an executor handle from the composition root. ChannelHost
//! provides scheduled RPC for foreign pollers. ChildHost starts arutd and awaits its
//! READY handshake. The final channel drop terminates the child and removes its
//! socket. No HTTP transport creates an executor.

#[cfg(unix)]
pub mod child;
pub mod hosting;
use arut_feature_chat::composer::authority::ComposerAuthority;
use arut_feature_chat::composer::service::ComposerServiceImpl;
use arut_feature_chat::ports::NativeIds;
use arut_feature_chat::service::ChatServiceImpl;
use arut_protocol::capability::v1::CapabilityServiceRouter;
use arut_protocol::capability_manifest::CapabilityServiceImpl;
use arut_protocol::chat::composer::v1::ComposerServiceRouter;
use arut_protocol::chat::v1::ChatServiceRouter;
use arut_rpc::{RpcRegistry, RpcService};
use axum::Router;
use std::path::PathBuf;
use std::sync::Arc;

pub fn app(data_path: PathBuf) -> Result<Router, String> {
    let directory = Arc::new(arut_storage::Directory::open(data_path).map_err(|e| e.to_string())?);
    let authority = Arc::new(ComposerAuthority::with_store(directory.clone()));
    let composer = Arc::new(ComposerServiceImpl::new(Arc::clone(&authority)));
    let log = directory.log("chat").map_err(|e| e.to_string())?;
    let chat = Arc::new(
        ChatServiceImpl::new(authority, Arc::new(log), Arc::new(NativeIds))
            .map_err(|e| e.to_string())?,
    );
    let composer_router: Arc<dyn RpcService> = Arc::new(ComposerServiceRouter::new(composer));
    let chat_router: Arc<dyn RpcService> = Arc::new(ChatServiceRouter::new(chat));
    let registry = RpcRegistry::default()
        .register(composer_router)
        .expect("composer RPC routes must be unique")
        .register(chat_router)
        .expect("chat RPC routes must be unique");
    let capabilities = Arc::new(CapabilityServiceImpl::new(registry.registrations()));
    let capability_router: Arc<dyn RpcService> =
        Arc::new(CapabilityServiceRouter::new(capabilities));
    let registry = Arc::new(
        registry
            .register(capability_router)
            .expect("capability RPC routes must be unique"),
    );

    Ok(arut_transport_connect_http::router(registry))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arut_feature_chat::composer::ComposerScope;
    use arut_protocol::capability::v1::{CapabilityServiceClient, GetCapabilitiesRequest};
    use arut_protocol::chat::composer::v1::{
        COMPOSER_SERVICE_DESCRIPTOR, ComposerServiceClient, ReplaceComposerRequest,
    };
    use arut_protocol::chat::v1::{ChatServiceClient, StartChatRequest};
    use arut_rpc::{Request, RpcChannel};
    use arut_transport_connect_http::HttpRpcChannel;
    use futures_util::StreamExt;

    fn wire_scope(scope: &ComposerScope) -> arut_protocol::chat::composer::v1::ComposerScope {
        arut_feature_chat::composer::service::scope_to_wire(scope)
    }

    #[tokio::test]
    async fn generated_clients_reach_nested_composer_and_chat_over_http() {
        let path =
            std::env::temp_dir().join(format!("arut-composer-http-test-{}.pb", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server_path = path.clone();
        let server = tokio::spawn(async move {
            axum::serve(listener, app(server_path).unwrap())
                .await
                .unwrap();
        });
        let channel: Arc<dyn RpcChannel> =
            Arc::new(HttpRpcChannel::new(format!("http://{address}")));
        let composer = ComposerServiceClient::remote(channel.clone());
        let chat = ChatServiceClient::remote(channel.clone());
        let capabilities = CapabilityServiceClient::remote(channel);

        composer
            .replace_composer(Request::new(ReplaceComposerRequest {
                scope: Some(wire_scope(&ComposerScope::pending("account"))),
                command_id: "draft".into(),
                authority_epoch: 1,
                base_revision: 0,
                text: "over HTTP".into(),
            }))
            .await
            .unwrap();
        let request = StartChatRequest {
            pending_scope_id: "account".into(),
            command_id: "start".into(),
            expected_revision: 1,
            text: "over HTTP".into(),
        };
        let first = chat
            .start_chat(Request::new(request.clone()))
            .await
            .unwrap()
            .message;
        let retry = chat
            .start_chat(Request::new(request))
            .await
            .unwrap()
            .message;
        assert_eq!(retry.chat_id, first.chat_id);
        assert_eq!(retry.messages, first.messages);
        let mut stream = composer
            .watch_composer(Request::new(
                arut_protocol::chat::composer::v1::WatchComposerRequest {
                    scope: Some(wire_scope(&ComposerScope::chat(first.chat_id.clone()))),
                    after_revision: 0,
                },
            ))
            .await
            .unwrap()
            .message;
        assert_eq!(
            stream
                .next()
                .await
                .unwrap()
                .unwrap()
                .snapshot
                .unwrap()
                .revision,
            0
        );
        composer
            .replace_composer(Request::new(ReplaceComposerRequest {
                scope: Some(wire_scope(&ComposerScope::chat(first.chat_id))),
                command_id: "stream-edit".into(),
                authority_epoch: 1,
                base_revision: 0,
                text: "streamed draft".into(),
            }))
            .await
            .unwrap();
        assert_eq!(
            stream.next().await.unwrap().unwrap().snapshot.unwrap().text,
            "streamed draft"
        );

        let manifest = capabilities
            .get_capabilities(Request::new(GetCapabilitiesRequest {}))
            .await
            .unwrap()
            .message
            .manifest
            .unwrap();
        assert_eq!(manifest.services.len(), 2);
        assert!(
            manifest
                .services
                .iter()
                .any(|service| service.package == COMPOSER_SERVICE_DESCRIPTOR.package)
        );
        server.abort();
        let _ = std::fs::remove_dir_all(path);
    }
}
