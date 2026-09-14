//! Parent-owned node and workspace construction with supplied chat clients.
//!
//! Cancellation follows the ownership tree: workspaces create child scopes for
//! conversations, and cancelling a parent cancels its descendants.
//!
//! Neither struct names a feature: a workspace hands out whatever client set it
//! was built over, and `product/session/src/feature.rs` says which member of
//! that set answers for which feature.

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
    /// The capability bundle or client set this node was constructed over.
    pub fn runtime(&self) -> &Arc<R> {
        &self.runtime
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
    /// The clients this workspace's node was constructed over. A feature set
    /// takes its own member out of them; nothing here names a feature.
    pub fn clients(&self) -> &R {
        self.node.runtime()
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
