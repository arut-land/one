//! # Local runtime
//!
//! `arutd` hosts chat and composer services over Connect HTTP or a private Unix
//! socket. Directory storage contains immutable chat facts, raw blobs, and local
//! draft recovery values. ARUT_DATA selects the directory; ARUT_SOCKET selects IPC;
//! ARUT_ADDRESS selects TCP; ARUT_STORAGE selects which implementation holds the
//! facts and the drafts.
//!
//! Which one backs a node is a composition-root choice, not a storage one: both
//! pass the same conformance suite, so `app` picks and everything above the port
//! is unaware. The directory tree stays the default until redb has run here.
//!
//! TokioSpawner receives an executor handle from the composition root. ChannelHost
//! provides scheduled RPC for foreign pollers. ChildHost starts arutd and awaits its
//! READY handshake. The final channel drop terminates the child and removes its
//! socket. No HTTP transport creates an executor.

mod blocking;
#[cfg(unix)]
pub mod child;
pub mod hosting;
use arut_feature_chat::composer::authority::ComposerAuthority;
use arut_feature_chat::composer::service::ComposerServiceImpl;
pub use arut_feature_chat::ports::NativeIds;
use arut_feature_chat::service::ChatServiceImpl;
use arut_protocol::capability::v1::CapabilityServiceRouter;
use arut_protocol::capability_manifest::CapabilityServiceImpl;
use arut_protocol::chat::composer::v1::ComposerServiceRouter;
use arut_protocol::chat::v1::ChatFact;
use arut_protocol::chat::v1::ChatServiceRouter;
use arut_rpc::{RpcRegistry, RpcService};
use arut_storage::{FactLog, KeyValue, Redb, StorageError};
use axum::Router;
use std::path::PathBuf;
use std::sync::Arc;

/// Which storage implementation this node's composition root hands the feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeStorage {
    /// A directory tree: one Protobuf record per file. The default until redb
    /// has run in front of real conversations.
    #[default]
    Directory,
    /// One redb file holding the fact log and the drafts beside it. This node
    /// owns that file exclusively while it runs.
    Redb,
}

impl NodeStorage {
    /// Reads the choice from `ARUT_STORAGE`; anything unrecognised is the default.
    pub fn from_env() -> Self {
        match std::env::var("ARUT_STORAGE").as_deref() {
            Ok("redb") => Self::Redb,
            _ => Self::default(),
        }
    }
}

pub fn app(data_path: PathBuf) -> Result<Router, StorageError> {
    app_with(data_path, NodeStorage::default())
}

pub fn app_with(data_path: PathBuf, storage: NodeStorage) -> Result<Router, StorageError> {
    let directory = Arc::new(arut_storage::Directory::open(&data_path)?);
    let lease = Arc::new(
        std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(data_path.join("node.lock"))?,
    );
    lease.try_lock().map_err(|error| {
        let error: std::io::Error = error.into();
        StorageError::from(error)
    })?;
    let (log, drafts): (Arc<dyn FactLog<ChatFact>>, Arc<dyn KeyValue>) = match storage {
        NodeStorage::Directory => (Arc::new(directory.log("chat")?), directory),
        NodeStorage::Redb => {
            let redb = Redb::open(data_path.join("node.redb"))?;
            (Arc::new(redb.log()), Arc::new(redb))
        }
    };
    let authority = Arc::new(ComposerAuthority::with_store(drafts));
    let composer = Arc::new(blocking::Blocking::new(
        ComposerServiceImpl::new(Arc::clone(&authority)),
        lease.clone(),
    ));
    let chat = Arc::new(blocking::Blocking::new(
        ChatServiceImpl::new(authority, log, Arc::new(NativeIds))?,
        lease,
    ));
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

    #[test]
    fn a_node_directory_has_one_owner_for_either_storage_choice() {
        for storage in [NodeStorage::Directory, NodeStorage::Redb] {
            let path = std::env::temp_dir()
                .join(format!("arut-exclusive-{storage:?}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            let first = app_with(path.clone(), storage).unwrap();
            assert!(app_with(path.clone(), storage).is_err());
            drop(first);
            drop(app_with(path.clone(), storage).unwrap());
            std::fs::remove_dir_all(path).unwrap();
        }
    }

    #[tokio::test]
    async fn either_storage_implementation_serves_the_same_node() {
        for storage in [NodeStorage::Directory, NodeStorage::Redb] {
            let path = std::env::temp_dir()
                .join(format!("arut-storage-{storage:?}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                axum::serve(listener, app_with(path, storage).unwrap())
                    .await
                    .unwrap();
            });
            let channel: Arc<dyn RpcChannel> =
                Arc::new(HttpRpcChannel::new(format!("http://{address}")));
            let chat = ChatServiceClient::remote(channel);
            let request = StartChatRequest {
                pending_scope_id: "account".into(),
                command_id: "01900000-0000-7000-8000-000000000001".into(),
                expected_revision: 0,
                text: String::new(),
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

            assert_eq!(retry.chat_id, first.chat_id, "{storage:?}");
            assert_eq!(first.messages.len(), 2, "{storage:?}");
            server.abort();
        }
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
            command_id: "01900000-0000-7000-8000-000000000001".into(),
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
