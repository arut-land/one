//! Parent-owned node and workspace construction with compile-time chat capabilities.
//!
//! Cancellation follows the ownership tree. Workspaces create child cancellation
//! scopes for conversations; cancelling a parent cancels its descendants.

use arut_protocol::chat::{composer::v1::ComposerServiceClient, v1::ChatServiceClient};
use arut_rpc::Cancellation;
use std::sync::Arc;

/// The capability bundle chat needs. A node without it has no chat scopes.
pub trait ChatRuntime: Send + Sync + 'static {
    fn chat_service(&self) -> ChatServiceClient;
    fn composer_service(&self) -> ComposerServiceClient;
}

#[derive(Clone)]
pub(crate) struct Services {
    pub(crate) chat: ChatServiceClient,
    pub(crate) composer: ComposerServiceClient,
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

    #[test]
    fn dropping_a_workspace_cancels_only_what_it_owns() {
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
    }

    #[test]
    fn cancelling_a_node_reaches_every_scope_beneath_it() {
        let node = Node::new("one".into(), Arc::new(()));
        let workspace = node.workspace("default".into());
        let conversation = workspace.conversation_cancellation();

        node.cancellation().cancel();

        assert!(workspace.cancellation().is_cancelled());
        assert!(conversation.is_cancelled());
    }
}
