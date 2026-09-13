//! Native persistence ports, Tokio drivers, and generic node assembly.
//!
//! The composition root chooses storage and composes its feature list. A node
//! lease remains held by the runtime and every active RPC dispatch. Blocking
//! storage dispatch runs on Tokio's blocking pool, including router future polling.
//! ChildHost supervises arutd; ScheduledChannel supports foreign pollers.

mod blocking;
#[cfg(unix)]
pub mod child;
pub mod hosting;
use arut_feature_chat::ports::{Clock, Drafts, IdSource, Persist};
use arut_rpc::{RpcRegistry, RpcService, Status};
pub use arut_runtime_host_polled::{NativeClock, NativeIds};
use arut_storage::{Directory, Fact, FactLog, KeyValue, Redb, StorageError};
use axum::Router;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeStorage {
    #[default]
    Directory,
    Redb,
}
impl NodeStorage {
    pub fn from_env() -> Self {
        match std::env::var("ARUT_STORAGE").as_deref() {
            Ok("redb") => Self::Redb,
            _ => Self::default(),
        }
    }
}

pub struct LocalRuntime {
    directory: Arc<Directory>,
    path: PathBuf,
    databases: Option<Mutex<HashMap<String, Redb>>>,
    drafts: Arc<dyn KeyValue>,
    _lease: std::fs::File,
}
impl LocalRuntime {
    /// `primary_log` assigns the existing node.redb file to its feature namespace.
    /// Additional namespaces get separate databases; directory names stay stable.
    pub fn open(
        path: PathBuf,
        storage: NodeStorage,
        primary_log: &str,
    ) -> Result<Self, StorageError> {
        let directory = Arc::new(Directory::open(&path)?);
        let lease = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path.join("node.lock"))?;
        lease
            .try_lock()
            .map_err(|error| StorageError::from(std::io::Error::from(error)))?;
        let (databases, drafts): (_, Arc<dyn KeyValue>) = match storage {
            NodeStorage::Directory => (None, directory.clone()),
            NodeStorage::Redb => {
                let database = Redb::open(path.join("node.redb"))?;
                (
                    Some(Mutex::new(HashMap::from([(
                        primary_log.into(),
                        database.clone(),
                    )]))),
                    Arc::new(database),
                )
            }
        };
        Ok(Self {
            directory,
            path,
            databases,
            drafts,
            _lease: lease,
        })
    }
}
impl IdSource for LocalRuntime {
    fn new_id(&self) -> String {
        NativeIds.new_id()
    }
}
impl Clock for LocalRuntime {
    fn now(&self) -> u64 {
        NativeClock.now()
    }
}
impl Drafts for LocalRuntime {
    fn drafts(&self) -> Arc<dyn KeyValue> {
        self.drafts.clone()
    }
}
impl<F: Fact> Persist<F> for LocalRuntime {
    fn log(&self, namespace: &str) -> Result<Arc<dyn FactLog<F>>, StorageError> {
        match &self.databases {
            None => Ok(Arc::new(self.directory.log(namespace)?)),
            Some(databases) => {
                let mut databases = databases.lock().expect("databases poisoned");
                if !databases.contains_key(namespace) {
                    let path = self.path.join(format!(
                        "log-{}.redb",
                        arut_storage::digest(namespace.as_bytes())
                    ));
                    databases.insert(namespace.into(), Redb::open(path)?);
                }
                Ok(Arc::new(databases[namespace].log()))
            }
        }
    }
}

pub struct Node;
impl Node {
    /// Roots flatten their feature routers here. Duplicate routes are errors.
    pub fn serve<R: Send + Sync + 'static>(
        runtime: Arc<R>,
        features: impl IntoIterator<Item = Arc<dyn RpcService>>,
    ) -> Result<Router, Status> {
        let mut registry = RpcRegistry::default();
        for router in features {
            registry =
                registry.register(Arc::new(blocking::Blocking::new(router, runtime.clone())))?;
        }
        let registry = arut_protocol::capability_manifest::with_capabilities(registry)?;
        Ok(arut_transport_connect_http::router(Arc::new(registry)))
    }
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

    fn app(data: PathBuf) -> Result<Router, StorageError> {
        app_with(data, NodeStorage::default())
    }
    fn app_with(data: PathBuf, storage: NodeStorage) -> Result<Router, StorageError> {
        let runtime = Arc::new(LocalRuntime::open(data, storage, "chat")?);
        let feature = arut_feature_chat::compose(runtime.clone())?;
        Ok(Node::serve(runtime, feature.routers()).unwrap())
    }

    fn wire_scope(scope: &ComposerScope) -> arut_protocol::chat::composer::v1::ComposerScope {
        scope.into()
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

    #[test]
    fn a_surviving_direct_composer_client_keeps_the_node_lease() {
        let path = std::env::temp_dir().join(format!("arut-client-lease-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let runtime =
            Arc::new(LocalRuntime::open(path.clone(), NodeStorage::Directory, "chat").unwrap());
        let feature = arut_feature_chat::compose(runtime.clone()).unwrap();
        let composer = feature.clients().composer;
        drop(feature);
        drop(runtime);
        assert!(LocalRuntime::open(path.clone(), NodeStorage::Directory, "chat").is_err());
        drop(composer);
        drop(LocalRuntime::open(path.clone(), NodeStorage::Directory, "chat").unwrap());
        std::fs::remove_dir_all(path).unwrap();
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
