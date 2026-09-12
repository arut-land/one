//! Parent-owned construction and compile-time service capabilities.
use arut_protocol::chat::{composer::v1::ComposerServiceClient, v1::ChatServiceClient};
use arut_watch::Watch;
use std::{collections::BTreeMap, sync::Arc};

/// The capability bundle chat needs. A node without it cannot build chat scopes.
pub trait ChatRuntime: Send + Sync + 'static {
    fn chat_service(&self) -> ChatServiceClient;
    fn composer_service(&self) -> ComposerServiceClient;
}

#[derive(Clone)]
pub struct Services {
    pub chat: ChatServiceClient,
    pub composer: ComposerServiceClient,
}
impl ChatRuntime for Services {
    fn chat_service(&self) -> ChatServiceClient {
        self.chat.clone()
    }
    fn composer_service(&self) -> ComposerServiceClient {
        self.composer.clone()
    }
}

pub struct Node<R> {
    pub runtime: Arc<R>,
    pub id: String,
    pub workspaces: Watch<Vec<String>>,
    config: BTreeMap<String, String>,
}
impl<R> Node<R> {
    pub fn new(id: String, runtime: Arc<R>, config: BTreeMap<String, String>) -> Arc<Self> {
        Arc::new(Self {
            id,
            runtime,
            workspaces: Watch::new(vec![]),
            config,
        })
    }
    pub fn workspace(
        self: &Arc<Self>,
        id: String,
        config: BTreeMap<String, String>,
    ) -> Arc<Workspace<R>> {
        self.workspaces.update(|ids| {
            if !ids.contains(&id) {
                ids.push(id.clone());
            }
        });
        Arc::new(Workspace {
            node: Arc::clone(self),
            id,
            config,
        })
    }
}

pub struct Workspace<R> {
    pub node: Arc<Node<R>>,
    pub id: String,
    config: BTreeMap<String, String>,
}
impl<R> Workspace<R> {
    pub fn setting(&self, key: &str) -> Option<&str> {
        self.config
            .get(key)
            .or_else(|| self.node.config.get(key))
            .map(String::as_str)
    }
}
impl<R: ChatRuntime> Workspace<R> {
    pub fn chat_service(&self) -> ChatServiceClient {
        self.node.runtime.chat_service()
    }
    pub fn composer_service(&self) -> ComposerServiceClient {
        self.node.runtime.composer_service()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workspace_keeps_its_node_and_inherits_configuration() {
        let node = Node::new(
            "one".into(),
            Arc::new(()),
            BTreeMap::from([("model".into(), "mock".into())]),
        );
        let weak = Arc::downgrade(&node);
        let workspace = node.workspace("default".into(), BTreeMap::new());
        assert_eq!(node.workspaces.get(), ["default"]);
        drop(node);
        assert_eq!(workspace.setting("model"), Some("mock"));
        assert!(weak.upgrade().is_some());
        drop(workspace);
        assert!(weak.upgrade().is_none());
    }
}
