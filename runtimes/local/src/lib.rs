//! Native persistence ports, Tokio drivers, and generic node assembly.
//!
//! The composition root composes features over redb storage. A node
//! lease remains held by the runtime and every active RPC dispatch. Blocking
//! storage dispatch runs on Tokio's blocking pool, including router future polling.
//! ChildHost supervises arutd; the `Scheduled` layer supports foreign pollers.

mod blocking;
#[cfg(unix)]
pub mod child;
pub mod hosting;
pub mod readiness;
use arut_feature_chat::ports::{Clock, Drafts, IdSource, Persist};
use arut_rpc::{RpcRegistry, RpcService, ServiceSet, Status};
pub use arut_runtime_host_polled::{NativeClock, NativeIds};
use arut_storage::{Fact, FactLog, KeyValue, Redb, StorageError};
use axum::Router;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub struct LocalRuntime {
    path: PathBuf,
    databases: Mutex<HashMap<String, Redb>>,
    drafts: Arc<dyn KeyValue>,
    _lease: std::fs::File,
}
impl LocalRuntime {
    /// `primary_log` assigns the existing node.redb file to its feature namespace.
    /// Additional namespaces get separate databases.
    pub fn open(path: PathBuf, primary_log: &str) -> Result<Self, StorageError> {
        std::fs::create_dir_all(&path)?;
        let lease = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path.join("node.lock"))?;
        lease
            .try_lock()
            .map_err(|error| StorageError::from(std::io::Error::from(error)))?;
        let database = Redb::open(path.join("node.redb"))?;
        let databases = Mutex::new(HashMap::from([(primary_log.into(), database.clone())]));
        let drafts = Arc::new(database);
        Ok(Self {
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
        let mut databases = self.databases.lock().map_err(|_| StorageError::Corrupt)?;
        let database = match databases.entry(namespace.into()) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let path = self.path.join(format!(
                    "log-{}.redb",
                    arut_storage::digest(namespace.as_bytes())
                ));
                entry.insert(Redb::open(path)?)
            }
        };
        Ok(Arc::new(database.log()))
    }
}

pub struct Node;
impl Node {
    /// Roots flatten their feature routers here and name the same services as
    /// `S`, which is what the manifest advertises. Duplicate routes are errors.
    ///
    /// # Errors
    /// Returns `AlreadyExists` if two routers claim the same procedure.
    pub fn serve<R: Send + Sync + 'static, S: ServiceSet>(
        runtime: Arc<R>,
        features: impl IntoIterator<Item = Arc<dyn RpcService>>,
    ) -> Result<Router, Status> {
        let mut registry = RpcRegistry::default();
        for router in features {
            registry =
                registry.register(Arc::new(blocking::Blocking::new(router, runtime.clone())))?;
        }
        let registry = arut_protocol::capability_manifest::with_capabilities::<S>(registry)?;
        Ok(arut_transport::router(Arc::new(registry)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arut_feature_chat::ChatServices;
    use arut_feature_chat::composer::ComposerScope;
    use arut_protocol::capability::v1::{CapabilityServiceClient, GetCapabilitiesRequest};
    use arut_protocol::chat::composer::v1::{
        COMPOSER_SERVICE_DESCRIPTOR, ComposerServiceClient, ReplaceComposerRequest,
    };
    use arut_protocol::chat::v1::{ChatServiceClient, StartChatRequest};
    use arut_rpc::{Request, RpcChannel};
    use arut_transport::HttpRpcChannel;
    use futures_util::StreamExt;

    fn app(data: PathBuf) -> Result<Router, StorageError> {
        let runtime = Arc::new(LocalRuntime::open(data, "chat")?);
        let feature = arut_feature_chat::compose(runtime.clone())?;
        Ok(Node::serve::<_, ChatServices>(runtime, feature.routers()).unwrap())
    }

    fn wire_scope(scope: &ComposerScope) -> arut_protocol::chat::composer::v1::ComposerScope {
        scope.into()
    }

    #[test]
    fn a_node_directory_has_one_owner() {
        {
            let path = std::env::temp_dir().join(format!("arut-exclusive-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            let first = app(path.clone()).unwrap();
            assert!(app(path.clone()).is_err());
            drop(first);
            drop(app(path.clone()).unwrap());
            std::fs::remove_dir_all(path).unwrap();
        }
    }

    #[test]
    fn a_surviving_direct_composer_client_keeps_the_node_lease() {
        let path = std::env::temp_dir().join(format!("arut-client-lease-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let runtime = Arc::new(LocalRuntime::open(path.clone(), "chat").unwrap());
        let feature = arut_feature_chat::compose(runtime.clone()).unwrap();
        let composer = feature.clients().composer;
        drop(feature);
        drop(runtime);
        assert!(LocalRuntime::open(path.clone(), "chat").is_err());
        drop(composer);
        drop(LocalRuntime::open(path.clone(), "chat").unwrap());
        std::fs::remove_dir_all(path).unwrap();
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
