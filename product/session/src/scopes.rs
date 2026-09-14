//! Parent-owned node and workspace construction with supplied chat clients.
//!
//! Cancellation follows the ownership tree: workspaces create child scopes for
//! conversations, and cancelling a parent cancels its descendants.

use arut_protocol::chat::{composer::v1::ComposerServiceClient, v1::ChatServiceClient};
use arut_rpc::Cancellation;
use std::sync::Arc;

pub struct Node<R> {
    runtime: Arc<R>,
    id: String,
    cancellation: Cancellation,
}
impl<R> Node<R> {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn new(id: String, runtime: Arc<R>) -> Arc<Self> {
        Arc::new(Self {
            runtime,
            id,
            cancellation: Cancellation::root(),
        })
    }
    pub fn cancellation(&self) -> &Cancellation {
        &self.cancellation
    }
    pub fn workspace(self: &Arc<Self>, id: String) -> Arc<Workspace<R>> {
        Arc::new(Workspace {
            cancellation: self.cancellation.child(),
            node: Arc::clone(self),
            id,
        })
    }
}

pub struct Workspace<R> {
    node: Arc<Node<R>>,
    id: String,
    cancellation: Cancellation,
}
impl<R> Workspace<R> {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn node(&self) -> &Arc<Node<R>> {
        &self.node
    }
    pub fn cancellation(&self) -> &Cancellation {
        &self.cancellation
    }
    /// The cancellation one conversation under this workspace owns.
    pub fn conversation_cancellation(&self) -> Arc<Cancellation> {
        Arc::new(self.cancellation.child())
    }
}
impl Workspace<arut_feature_chat::ChatClients> {
    pub fn chat_service(&self) -> ChatServiceClient {
        self.node.runtime.chat.clone()
    }
    pub fn composer_service(&self) -> ComposerServiceClient {
        self.node.runtime.composer.clone()
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

    #[test]
    fn cancellation_follows_ownership_down_and_never_sideways() {
        let node = Node::new("one".into(), Arc::new(()));
        let workspace = node.workspace("default".into());
        let sibling = node.workspace("other".into());
        let conversation = workspace.conversation_cancellation();
        let (workspace_token, conversation_token, sibling_token) = (
            workspace.cancellation().token(),
            conversation.token(),
            sibling.cancellation().token(),
        );

        drop(workspace);

        assert!(workspace_token.is_cancelled());
        assert!(conversation_token.is_cancelled());
        assert!(!sibling_token.is_cancelled());
        assert!(!node.cancellation().is_cancelled());

        node.cancellation().cancel();

        assert!(sibling.cancellation().is_cancelled());
        assert!(sibling_token.is_cancelled());
    }
}
