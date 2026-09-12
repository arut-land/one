//! Parent-owned construction and compile-time service capabilities.
use arut_protocol::chat::{composer::v1::ComposerServiceClient, v1::ChatServiceClient};
use std::sync::Arc;

/// The capability bundle chat needs. A node without it has no chat scopes.
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
}
impl<R> Node<R> {
    pub fn new(id: String, runtime: Arc<R>) -> Arc<Self> {
        Arc::new(Self { runtime, id })
    }
    pub fn workspace(self: &Arc<Self>, id: String) -> Arc<Workspace<R>> {
        Arc::new(Workspace {
            node: Arc::clone(self),
            id,
        })
    }
}

pub struct Workspace<R> {
    pub node: Arc<Node<R>>,
    pub id: String,
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
    fn a_workspace_keeps_the_node_that_made_it_alive() {
        let node = Node::new("one".into(), Arc::new(()));
        let weak = Arc::downgrade(&node);
        let workspace = node.workspace("default".into());
        drop(node);
        assert_eq!(workspace.node.id, "one");
        assert!(weak.upgrade().is_some());
        drop(workspace);
        assert!(weak.upgrade().is_none());
    }
}
