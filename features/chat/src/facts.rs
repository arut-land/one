use arut_authority::Projection;
use arut_protocol::chat::v1::{ChatFact, Conversation, OperationPhase};

/// The durable projection of accepted chat facts; snapshots persist it as a row.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ChatProjection {
    #[prost(message, repeated, tag = "1")]
    pub conversations: Vec<Conversation>,
    #[prost(string, repeated, tag = "2")]
    pub completed_operations: Vec<String>,
    #[prost(btree_map = "string, uint64", tag = "3")]
    pub consumed_drafts: std::collections::BTreeMap<String, u64>,
}

impl ChatProjection {
    pub fn conversation(&self, chat_id: &str) -> Option<&Conversation> {
        self.conversations
            .iter()
            .find(|conversation| conversation.id == chat_id)
    }
    pub fn apply(&mut self, fact: &ChatFact) {
        if let Some(revision) = fact.pending_revision {
            self.consumed_drafts
                .entry(fact.pending_scope_id.clone())
                .and_modify(|current| *current = (*current).max(revision))
                .or_insert(revision);
        }
        match self
            .conversations
            .iter_mut()
            .find(|conversation| conversation.id == fact.chat_id)
        {
            Some(conversation) => conversation.messages.extend(fact.messages.iter().cloned()),
            None => self.conversations.push(Conversation {
                id: fact.chat_id.clone(),
                messages: fact.messages.clone(),
            }),
        }
        self.completed_operations.extend(
            fact.operations
                .iter()
                .filter(|operation| operation.phase() == OperationPhase::Completed)
                .map(|operation| operation.operation_id.clone()),
        );
    }
}

impl Projection for ChatProjection {
    type Fact = ChatFact;
    fn reduce(&mut self, fact: &ChatFact) {
        self.apply(fact);
    }
}
