//! Parent-owned construction and compile-time service capabilities.
use arut_feature_chat::product::ChatClient;
use arut_protocol::chat::{composer::v1::ComposerServiceClient, v1::ChatServiceClient};
use arut_watch::Watch;
use std::{collections::BTreeMap, sync::Arc};

pub trait NodeRuntime: Send + Sync + 'static {}
impl<T: Send + Sync + 'static> NodeRuntime for T {}
pub trait ChatServices {
    fn chat_service(&self) -> ChatServiceClient;
    fn composer_service(&self) -> ComposerServiceClient;
}
pub trait ChatRuntime: NodeRuntime + ChatServices {}
impl<T: NodeRuntime + ChatServices> ChatRuntime for T {}

#[derive(Clone)]
pub struct Services {
    pub chat: ChatServiceClient,
    pub composer: ComposerServiceClient,
}
impl ChatServices for Services {
    fn chat_service(&self) -> ChatServiceClient {
        self.chat.clone()
    }
    fn composer_service(&self) -> ComposerServiceClient {
        self.composer.clone()
    }
}

pub struct Node<R: NodeRuntime> {
    pub runtime: Arc<R>,
    pub id: String,
    pub workspaces: Watch<Vec<String>>,
    config: BTreeMap<String, String>,
}
impl<R: NodeRuntime> Node<R> {
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

pub struct Workspace<R: NodeRuntime> {
    pub node: Arc<Node<R>>,
    pub id: String,
    config: BTreeMap<String, String>,
}
impl<R: NodeRuntime> Workspace<R> {
    pub fn setting(&self, key: &str) -> Option<&str> {
        self.config
            .get(key)
            .or_else(|| self.node.config.get(key))
            .map(String::as_str)
    }
}
impl<R: ChatRuntime> Workspace<R> {
    pub fn conversation(self: &Arc<Self>, client: ChatClient) -> Arc<Conversation<R>> {
        Arc::new(Conversation {
            workspace: Arc::clone(self),
            client,
        })
    }
}

pub struct Conversation<R: ChatRuntime> {
    pub workspace: Arc<Workspace<R>>,
    pub client: ChatClient,
}
impl<R: ChatRuntime> Conversation<R> {
    pub fn operation(self: &Arc<Self>, id: String) -> Operation<R> {
        Operation {
            conversation: Arc::clone(self),
            id,
            progress: Watch::new(0),
        }
    }
}
pub struct Operation<R: ChatRuntime> {
    pub conversation: Arc<Conversation<R>>,
    pub id: String,
    pub progress: Watch<u64>,
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
        let workspace = node.workspace("default".into(), Default::default());
        assert_eq!(node.workspaces.get(), ["default"]);
        drop(node);
        assert_eq!(workspace.setting("model"), Some("mock"));
        assert!(weak.upgrade().is_some());
        drop(workspace);
        assert!(weak.upgrade().is_none());
    }
}
